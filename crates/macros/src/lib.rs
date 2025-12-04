//! Procedural macros for Enya observability.
//!
//! This crate provides attribute macros for instrumenting async functions
//! with Tokio TaskMonitor metrics collection. Monitors are automatically
//! registered with Enya and metrics are collected periodically.
//!
//! # Example
//!
//! ```rust,ignore
//! use enya::macros::monitor;
//!
//! // Basic usage - task name defaults to function name "my_background_task"
//! #[monitor]
//! async fn my_background_task() {
//!     // Task work here
//!     // Metrics are automatically collected by Enya
//! }
//!
//! // With custom name for metrics tagging
//! #[monitor(name = "custom_task")]
//! async fn named_task() {
//!     // Task work here
//! }
//!
//! // With slow poll threshold (alerts when polls exceed threshold)
//! #[monitor(slow_poll = "100ms")]
//! async fn query_handler() {
//!     // Query processing
//! }
//!
//! // Combined options
//! #[monitor(name = "db_query", slow_poll = "50us", long_delay = "1ms")]
//! async fn execute_query() {
//!     // Database query
//! }
//! ```
//!
//! # Collected Metrics
//!
//! The following metrics are collected for each instrumented task:
//!
//! - `task.poll.count` - Total number of polls
//! - `task.poll.duration` - Time spent polling (histogram)
//! - `task.poll.slow_count` - Polls exceeding slow_poll threshold
//! - `task.idle.duration` - Time spent waiting for wakeups
//! - `task.scheduled.duration` - Time from wake to poll (scheduling delay)
//! - `task.scheduled.long_count` - Delays exceeding long_delay threshold

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    Error, Expr, ExprLit, ItemFn, Lit, Meta, Result, Token,
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
    punctuated::Punctuated,
};

/// Configuration options for the monitor macro.
#[derive(Default)]
struct MonitorArgs {
    /// Custom name for the task monitor (defaults to function name).
    name: Option<String>,
    /// Threshold for categorizing polls as "slow" (e.g., "50us", "1ms").
    slow_poll: Option<String>,
    /// Threshold for categorizing scheduling delays as "long" (e.g., "50us", "1ms").
    long_delay: Option<String>,
}

impl Parse for MonitorArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut args = MonitorArgs::default();

        if input.is_empty() {
            return Ok(args);
        }

        let metas: Punctuated<Meta, Token![,]> = Punctuated::parse_terminated(input)?;

        for meta in metas {
            match &meta {
                Meta::NameValue(nv) => {
                    let ident = nv
                        .path
                        .get_ident()
                        .ok_or_else(|| Error::new_spanned(&nv.path, "expected identifier"))?;

                    let value = match &nv.value {
                        Expr::Lit(ExprLit {
                            lit: Lit::Str(s), ..
                        }) => s.value(),
                        _ => return Err(Error::new_spanned(&nv.value, "expected string literal")),
                    };

                    match ident.to_string().as_str() {
                        "name" => args.name = Some(value),
                        "slow_poll" => args.slow_poll = Some(value),
                        "long_delay" => args.long_delay = Some(value),
                        other => {
                            return Err(Error::new_spanned(
                                ident,
                                format!("unknown argument: {other}"),
                            ));
                        }
                    }
                }
                _ => {
                    return Err(Error::new_spanned(
                        &meta,
                        "expected name = \"value\" format",
                    ));
                }
            }
        }

        Ok(args)
    }
}

/// Parse a duration string like "50us", "1ms", "100ns" into a Duration expression.
fn parse_duration_expr(s: &str) -> Result<TokenStream2> {
    let s = s.trim();

    // Try parsing different suffixes
    if let Some(num) = s.strip_suffix("ns") {
        let n: u64 = num
            .trim()
            .parse()
            .map_err(|_| Error::new(proc_macro2::Span::call_site(), "invalid nanoseconds value"))?;
        return Ok(quote! { ::std::time::Duration::from_nanos(#n) });
    }
    if let Some(num) = s.strip_suffix("us") {
        let n: u64 = num.trim().parse().map_err(|_| {
            Error::new(proc_macro2::Span::call_site(), "invalid microseconds value")
        })?;
        return Ok(quote! { ::std::time::Duration::from_micros(#n) });
    }
    if let Some(num) = s.strip_suffix("µs") {
        let n: u64 = num.trim().parse().map_err(|_| {
            Error::new(proc_macro2::Span::call_site(), "invalid microseconds value")
        })?;
        return Ok(quote! { ::std::time::Duration::from_micros(#n) });
    }
    if let Some(num) = s.strip_suffix("ms") {
        let n: u64 = num.trim().parse().map_err(|_| {
            Error::new(proc_macro2::Span::call_site(), "invalid milliseconds value")
        })?;
        return Ok(quote! { ::std::time::Duration::from_millis(#n) });
    }
    if let Some(num) = s.strip_suffix('s') {
        let n: u64 = num
            .trim()
            .parse()
            .map_err(|_| Error::new(proc_macro2::Span::call_site(), "invalid seconds value"))?;
        return Ok(quote! { ::std::time::Duration::from_secs(#n) });
    }

    Err(Error::new(
        proc_macro2::Span::call_site(),
        "invalid duration format, expected: 50us, 1ms, 100ns, or 1s",
    ))
}

/// Instrument an async function with Tokio TaskMonitor for metrics collection.
///
/// This attribute macro wraps async functions to track their execution metrics
/// using `tokio_metrics::TaskMonitor`. The monitor is automatically registered
/// with Enya on first call, and metrics are collected periodically.
///
/// # Tracked Metrics
///
/// - Poll durations and slow poll counts
/// - Scheduling delays (time from wake to poll)
/// - Idle duration (time spent waiting for wakeups)
/// - Total poll counts
///
/// # Arguments
///
/// - `name = "..."` - Custom name for the task (defaults to function name)
/// - `slow_poll = "..."` - Threshold for slow polls (e.g., "50us", "1ms")
/// - `long_delay = "..."` - Threshold for long scheduling delays (e.g., "50us", "1ms")
///
/// # Example
///
/// ```rust,ignore
/// use enya::macros::monitor;
///
/// #[monitor]
/// async fn background_worker() {
///     // Work... metrics collected automatically
/// }
///
/// #[monitor(name = "db_query", slow_poll = "100us")]
/// async fn execute_query() -> Result<(), Error> {
///     // Query...
///     Ok(())
/// }
/// ```
///
/// # How It Works
///
/// The macro generates a static `TaskMonitor` and wraps the function body
/// with `monitor.instrument(...)`. On first invocation, the monitor is
/// registered with Enya's global registry for automatic metrics collection.
#[proc_macro_attribute]
pub fn monitor(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as MonitorArgs);
    let mut input_fn = parse_macro_input!(input as ItemFn);

    match expand_monitor(args, &mut input_fn) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn expand_monitor(args: MonitorArgs, input_fn: &mut ItemFn) -> Result<TokenStream2> {
    // Ensure the function is async
    if input_fn.sig.asyncness.is_none() {
        return Err(Error::new_spanned(
            &input_fn.sig,
            "monitor can only be applied to async functions",
        ));
    }

    let fn_name = &input_fn.sig.ident;
    // Use custom name if provided, otherwise use the function name
    let monitor_name = args.name.unwrap_or_else(|| fn_name.to_string());

    // Generate the static monitor name (uses function name for uniqueness)
    let monitor_static_name = syn::Ident::new(
        &format!("__ENYA_TASK_MONITOR_{}", fn_name.to_string().to_uppercase()),
        fn_name.span(),
    );

    // Store the monitor name for metrics emission
    let monitor_name_str = &monitor_name;

    // Build the monitor constructor based on thresholds
    let monitor_constructor = match (&args.slow_poll, &args.long_delay) {
        (None, None) => {
            quote! { ::tokio_metrics::TaskMonitor::new() }
        }
        (Some(slow), None) => {
            let slow_dur = parse_duration_expr(slow)?;
            quote! {
                ::tokio_metrics::TaskMonitor::builder()
                    .with_slow_poll_threshold(#slow_dur)
                    .build()
            }
        }
        (None, Some(long)) => {
            let long_dur = parse_duration_expr(long)?;
            quote! {
                ::tokio_metrics::TaskMonitor::builder()
                    .with_long_delay_threshold(#long_dur)
                    .build()
            }
        }
        (Some(slow), Some(long)) => {
            let slow_dur = parse_duration_expr(slow)?;
            let long_dur = parse_duration_expr(long)?;
            quote! {
                ::tokio_metrics::TaskMonitor::builder()
                    .with_slow_poll_threshold(#slow_dur)
                    .with_long_delay_threshold(#long_dur)
                    .build()
            }
        }
    };

    // Extract the original function body
    let original_body = &input_fn.block;

    // Generate the registration flag name
    let registered_flag_name = syn::Ident::new(
        &format!(
            "__ENYA_TASK_MONITOR_REGISTERED_{}",
            fn_name.to_string().to_uppercase()
        ),
        fn_name.span(),
    );

    // Create the new instrumented body that registers on first call
    let new_body: syn::Block = parse_quote! {{
        // Register the monitor with Enya on first call
        static #registered_flag_name: ::std::sync::Once = ::std::sync::Once::new();
        #registered_flag_name.call_once(|| {
            ::enya::task_registry::register_task_monitor(#monitor_name_str, &*#monitor_static_name);
        });

        #monitor_static_name.instrument(async move #original_body).await
    }};

    input_fn.block = Box::new(new_body);

    // Get visibility, attributes, and signature
    let vis = &input_fn.vis;
    let attrs = &input_fn.attrs;
    let sig = &input_fn.sig;
    let block = &input_fn.block;

    // Generate the output with static monitor (no accessor needed - auto-registered)
    let fn_name_str = fn_name.to_string();
    let expanded = quote! {
        #[doc = concat!("Static TaskMonitor for the `", #fn_name_str, "` function.")]
        #[doc = concat!("Task name: `", #monitor_name_str, "`")]
        static #monitor_static_name: ::std::sync::LazyLock<::tokio_metrics::TaskMonitor> =
            ::std::sync::LazyLock::new(|| #monitor_constructor);

        #(#attrs)*
        #vis #sig #block
    };

    Ok(expanded)
}

/// Instrument an async block for spawning with TaskMonitor tracking.
///
/// This macro is designed for use with `tokio::spawn` to instrument
/// spawned tasks with a given monitor.
///
/// # Example
///
/// ```rust,ignore
/// use enya_macros::spawn_monitored;
/// use tokio_metrics::TaskMonitor;
///
/// let monitor = TaskMonitor::new();
///
/// tokio::spawn(spawn_monitored!(monitor, async {
///     // Task work
/// }));
/// ```
#[proc_macro]
pub fn spawn_monitored(input: TokenStream) -> TokenStream {
    let input2: TokenStream2 = input.into();

    // Parse as: monitor, async { ... }
    let expanded = quote! {
        {
            let (__monitor, __task) = (#input2);
            __monitor.instrument(__task)
        }
    };

    expanded.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration_ns() {
        let result = parse_duration_expr("100ns").unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_parse_duration_us() {
        let result = parse_duration_expr("50us").unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_parse_duration_ms() {
        let result = parse_duration_expr("10ms").unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_parse_duration_s() {
        let result = parse_duration_expr("1s").unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_parse_duration_invalid() {
        let result = parse_duration_expr("invalid");
        assert!(result.is_err());
    }
}
