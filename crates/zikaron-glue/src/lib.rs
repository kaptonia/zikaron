#![forbid(unsafe_code)]

pub const V2: &str = "V2";

pub fn seam_v2() {
    zikaron::trace::mark(V2);
}

pub mod addressed;
pub mod anchoredon;
pub mod anchorproofs;
pub mod basis;
pub mod book;
pub mod container;
pub mod door;
pub mod doorrules;
pub mod doorverbs;
pub mod extract;
pub mod face;
pub mod facegate;
pub mod grantfile;
pub mod kitnames;
pub mod landing;
pub mod mirror;
pub mod names;
pub mod notices;
pub mod onefile;
pub mod pack;
pub mod read;
pub mod recordcheck;
pub mod recording;
pub mod retraction;
pub mod sealed;
pub mod select;
pub mod tidy;
