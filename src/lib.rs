#![allow(unused_imports)]

use color_eyre::eyre::{self, WrapErr};
use tracing::{event, info, instrument, span, warn, Level};

mod bonusly;
pub use bonusly::*;

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use pretty_assertions::{assert_eq, assert_ne};

    use super::*;
}
