//! As a library crate, `img_dup` provides tools for searching for images, hashing them in
//! parallel, and collating their hashes to find near or complete duplicates.
//!
//!
#![feature(collections, convert, fs_walk, std_misc)]

extern crate rustc_serialize;
extern crate img_hash;
extern crate image;
extern crate num_cpus;

mod compare;
mod img;
mod serialize;
mod threaded;

use compare::ImageManager;

pub use compare::UniqueImage;

use img::{
	ImgResults,
	ImgStatus,
	HashSettings,
};

pub use img::Image;

use threaded::ThreadedSession;

use img_hash::HashType;

use std::borrow::ToOwned;
use std::convert::AsRef;
use std::fs::{self, DirEntry};
use std::io;
use std::path::{Path, PathBuf};

pub static DEFAULT_EXTS: &'static [&'static str] = &["jpg", "png", "gif"];

/// A helper struct for searching for image files within a directory.
pub struct ImageSearch<'a> {
    /// The directory to search
    pub dir: &'a Path,
    /// If the search should be recursive (visit subdirectories)
    pub recursive: bool,
    /// The extensions to match.
    pub exts: Vec<&'a str>,
}

impl<'a> ImageSearch<'a> {
    /// Initiate a search builder with the base search directory.
    /// Starts with a copy of `DEFAULT_EXTS` for the list of file extensions,
    /// and `recursive` set to `false`.
    pub fn with_dir<P: AsRef<Path>>(dir: &'a P) -> ImageSearch<'a> {
        ImageSearch {
            dir: dir.as_ref(),
            recursive: false,
            exts: DEFAULT_EXTS.to_owned(),
        }
    }

    pub fn recursive(&mut self, recursive: bool) -> &mut ImageSearch<'a> {
        self.recursive = recursive;
        self
    }

    /// Add an extension to the list on `self`,
    /// returning `self` for method chaining
    pub fn ext(&mut self, ext: &'a str) -> &mut ImageSearch<'a> {
        self.exts.push(ext);
        self
    }

    /// Add all the extensions from `exts` to `self,
    /// returning `self` for method chaining
    pub fn exts(&mut self, exts: &[&'a str]) -> &mut ImageSearch<'a> {
        self.exts.push_all(exts);
        self
    }

    /// Searche `self.dir` for images with extensions contained in `self.exts`,
    /// recursing into subdirectories if `self.recursive` is set to `true`.
    ///
    /// Returns a vector of all found images as paths.
    ///
    /// Any I/O errors during searching are safely filtered out.
    pub fn search(self) -> io::Result<Vec<PathBuf>> {
        /// Generic to permit code reuse
        fn do_filter<I: Iterator<Item=io::Result<DirEntry>>>(iter: I, exts: &[&str]) -> Vec<PathBuf> {
                iter.filter_map(|res| res.ok())
                    .map(|entry| entry.path())
                    .filter(|path|
                        path.extension()
                            .and_then(|s| s.to_str())
                            .map(|ext| exts.contains(&ext))
                            .unwrap_or(false)
                    )
                    .collect()
        }

        // `match` instead of `if` for clarity
        let paths = match self.recursive {
            false => do_filter(try!(fs::read_dir(self.dir)), &self.exts),
            true => do_filter(try!(fs::walk_dir(self.dir)), &self.exts),
        };

        Ok(paths)
    }
}

pub const DEAFULT_HASH_SIZE: u32 = 16;
pub const DEFAULT_HASH_TYPE: HashType = HashType::Gradient;
pub const DEFAULT_THRESHOLD: u32 = 2;

/// A builder struct for bootstrapping an `img_dup` session.
pub struct SessionBuilder {

    /// The images to hash and compare.
    pub images: Vec<PathBuf>,

    /// The size of the hash to use.
    ///
    /// See the `HashType` documentation for the actual size
    /// of a hash generated by each hash type.
    pub hash_size: u32,

    /// The type of the hash to use. See `HashType` for more information.
    pub hash_type: HashType,
}

macro_rules! setter {
    ($field:ident: $field_ty:ty) => (
        /// Set this field on `self`, returning `self` for method chaining.
        pub fn $field<'a>(&'a mut self, $field: $field_ty) -> &mut Self {
            self.$field = $field;
            self
        }
    )
}

impl SessionBuilder {
    /// Construct a `SessionBuilder` instance from the vector of paths,
    /// supplying values from the `DEFAULT_*` constants for the other fields.
    ///
    /// To search for images, use the `ImageSearch` struct.
    pub fn from_images(images: Vec<PathBuf>) -> SessionBuilder {
        SessionBuilder {
            images: images,
            hash_size: DEAFULT_HASH_SIZE,
            hash_type: DEFAULT_HASH_TYPE,
        }
    }

    setter! { hash_size: u32 }
    setter! { hash_type: HashType }

    /// Spawn an `img_dup` session, using `threads` if supplied,
    /// or the number of CPUs as reported by the OS otherwise (recommended).
    ///
    /// ### Note
    /// Regardless of the `threads` value, an additional thread will be used for result collation.
    ///
    /// ### Panics
    /// If `threads` is `Some(value)` and `value == 0`.
    ///
    /// If `threads` is `None` and this method panics, then for some reason `std::os::num_cpus()`
    /// returned 0, which is probably bad.
    pub fn process_multithread(self, threads: Option<usize>) -> ThreadedSession {
        let (settings, images) = self.recombine();
        ThreadedSession::process_multithread(threads, settings, images)
    } 

    /// Do all the processing and collation on the current thread and return the result directly.
    ///
    /// **Not** recommended unless avoiding extra threads altogether is somehow desirable.
    pub fn process_local(self) -> ImgResults {
        let (settings, images) = self.recombine();

        let mut results: Vec<_> = images.into_iter()
			.map(|img| ImgStatus::Unhashed(img))
			.collect();

		let _ = results.iter_mut().map(|img| img.hash(settings)).last();

		ImgResults::from_statuses(results)
    }

    fn recombine(self) -> (HashSettings, Vec<PathBuf>) {
        let hash_settings = HashSettings {
            hash_size: self.hash_size,
            hash_type: self.hash_type,
        };

        (hash_settings, self.images)
    }
}

pub fn find_uniques(images: Vec<Image>, threshold: u32) -> Vec<UniqueImage> {
	let mut mgr = ImageManager::new(threshold);
	mgr.add_all(images);
	mgr.into_vec()
}
