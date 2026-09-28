//! Helpers for redacting values in [`insta`] snapshots.
//!
//! Each helper validates the value it replaces, failing the test when the
//! snapshot content does not match the expected value.

use std::fmt::Display;

use assert2::check;
use insta::internals::Content;
use insta::internals::ContentPath;

/// Create an [`insta`] redaction that replaces the string form of `value` with
/// `[replacement]`.
///
/// The redaction checks that the content being redacted equals `value` before
/// substituting it.
#[allow(impl_trait_overcaptures)]
pub fn create_insta_redaction<T: Display>(
	value: T,
	replacement: &str,
) -> impl Fn(Content, ContentPath) -> String + Clone + 'static {
	let replacement = format!("[{replacement}]");
	let value = value.to_string();

	move |content: Content, _: ContentPath| {
		let content_value = content.as_str().unwrap();
		check!(
			content_value == &value,
			"redacated content value is not valid: {replacement}"
		);

		replacement.clone()
	}
}

/// Create an [`insta`] redaction for unsigned integer content, replacing
/// `value` with `[replacement]`.
///
/// Unlike [`create_insta_redaction`] this reads the content as a `u128`, so it
/// must only be used on snapshots containing integers.
#[allow(impl_trait_overcaptures)]
pub fn create_insta_redaction_u128<T: Display>(
	value: T,
	replacement: &str,
) -> impl Fn(Content, ContentPath) -> String + Clone + 'static {
	let replacement = format!("[{replacement}]");
	let value = value.to_string();

	move |content: Content, _: ContentPath| {
		let content_value = content.as_u128().unwrap().to_string();
		check!(
			&content_value == &value,
			"redacted int value is not valid: {replacement}"
		);

		replacement.clone()
	}
}
