//! Warcat: WARC Archiving Tool
//!
//! This crate provides both a library API and a binary CLI application.
//! The library can be used to read and write WARC files and
//! as well perform functions provided by the binary.
//!
//! In general cases, users working with WARC files do not need to program
//! directly with the library. The CLI application (the tool portion) is
//! designed to be part of a Unix-style pipeline. This documentation is for
//! the library portion. For details on the CLI, see the
//! [user guide](https://warcat-rs.readthedocs.io/).
//!
//! The main entrypoints to this library is [`warc::Decoder`] and [`warc::Encoder`].

#![cfg_attr(docsrs, feature(doc_auto_cfg))]

pub mod compress;
pub mod dataseq;
pub mod digest;
pub mod error;
pub mod extract;
pub mod fields;
pub mod header;
pub mod http;
pub mod io;
pub mod parse;
pub(crate) mod util;
pub mod verify;
pub mod warc;

#[cfg(feature = "app")]
#[doc(hidden)]
pub mod app;
