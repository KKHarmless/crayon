//! The root module of the game engine. It binds all the sub-system in a central place.
//!
//! # Application
//!

pub mod settings;
pub mod engine;
pub mod window;
pub mod input;
pub mod application;
pub mod errors;
pub mod event;

pub use self::settings::Settings;
pub use self::application::{Application, ApplicationInstance};
pub use self::event::{KeyboardButton, MouseButton};