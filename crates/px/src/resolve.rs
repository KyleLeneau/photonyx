use anyhow::Result;

/// Internal implementation — call via the [`first_some!`] macro instead.
#[allow(unused)]
pub(crate) fn resolve_chain<'a, T>(
    strategies: Vec<Box<dyn FnOnce() -> Result<Option<T>> + 'a>>,
) -> Result<Option<T>> {
    for strategy in strategies {
        if let Some(value) = strategy()? {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

/// Try each strategy in order, returning the first `Some` result.
///
/// Strategies are closures returning `Result<Option<T>>`, called lazily — the next is only
/// invoked if the previous returned `Ok(None)`. Propagates `Err` immediately.
///
/// # Example
///
/// ```ignore
/// let filter = first_some![
///     || Ok(args.filter.clone()),
///     || detect_filter_from_fits(&obs_path),
///     || detect_filter_from_filename(&obs_path),
///     || prompt_filter(),
/// ]?;
/// ```
#[allow(unused)]
macro_rules! first_some {
    ($($strategy:expr),+ $(,)?) => {{
        // A typed helper so Rust can infer T from context without an explicit `as` cast.
        fn coerce<'a, T>(f: impl FnOnce() -> anyhow::Result<Option<T>> + 'a) -> Box<dyn FnOnce() -> anyhow::Result<Option<T>> + 'a> {
            Box::new(f)
        }
        $crate::resolve::resolve_chain(vec![$(coerce($strategy),)+])
    }};
}

#[allow(unused)]
pub(crate) use first_some;
