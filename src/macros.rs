/// Create an axum `Response` struct containing the specified MIME type and body.
///
/// # Arguments
///
/// * `$a`: The MIME type of the response
/// * `$b`: The body of the response, specified as a `Vec<u8>`
///
/// # Returns
///
/// A `Response` struct containing the specified content type and body.
///
/// # Example
///
/// ```norun
/// response!(FONT_WOFF, store.font)
/// ```
///
/// will be
///
/// ```norun
/// (
///     [(header::CONTENT_TYPE, HeaderValue::from_static(FONT_WOFF.as_ref()))],
///     (store.font).clone(),
/// ).into_response()
/// ```

macro_rules! response {
    ($a:expr, $b:expr) => {
        // Create a tuple containing the content type header and the body of the response.
        (
            [(header::CONTENT_TYPE, HeaderValue::from_static($a.as_ref()))],
            $b.clone(),
        )
            // Convert the tuple into a `Response` struct.
            .into_response()
    };
}

// Export the macro
pub(crate) use response;
