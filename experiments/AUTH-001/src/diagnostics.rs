//! Bounded diagnostics: no headers, response content, peer identity or raw errors.
use std::{error::Error, fmt, io};

use bytes::Bytes;
use hyper::StatusCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExchangeFailure {
    Request,
    Body,
}

#[derive(Debug, PartialEq, Eq)]
enum ExchangeOutcome {
    Complete { status: u16, body_bytes: usize },
    Failed(ExchangeFailure),
}

#[derive(Debug, PartialEq, Eq)]
struct DriverFailure {
    during_shutdown: bool,
    io_kind: Option<io::ErrorKind>,
}

#[derive(Debug)]
struct RequestFailure {
    exchange: ExchangeOutcome,
    driver: Option<DriverFailure>,
}

impl fmt::Display for RequestFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HTTP exchange={:?}; driver={:?}",
            self.exchange, self.driver
        )
    }
}

impl Error for RequestFailure {}

fn driver_failure(error: &hyper::Error) -> DriverFailure {
    DriverFailure {
        during_shutdown: error.is_shutdown(),
        io_kind: underlying_io_kind(error),
    }
}

fn underlying_io_kind(error: &(dyn Error + 'static)) -> Option<io::ErrorKind> {
    let mut source: &(dyn Error + 'static) = error;
    // Bound traversal even for an unexpected cyclic upstream error chain.
    for _ in 0..8 {
        if let Some(error) = source.downcast_ref::<io::Error>() {
            return Some(error.kind());
        }
        match source.source() {
            Some(next) => source = next,
            None => break,
        }
    }
    None
}

type ExchangeResult = Result<(StatusCode, Bytes), ExchangeFailure>;

#[cfg(test)]
mod shutdown_test;

pub(crate) fn finish(
    exchange: ExchangeResult,
    driver: Result<(), hyper::Error>,
) -> io::Result<(StatusCode, Bytes)> {
    settle(exchange, driver.err().as_ref().map(driver_failure))
}

fn settle(
    exchange: ExchangeResult,
    driver: Option<DriverFailure>,
) -> io::Result<(StatusCode, Bytes)> {
    match (exchange, driver) {
        (Ok(value), None) => Ok(value),
        (exchange, driver) => {
            let exchange = match exchange {
                Ok((status, body)) => ExchangeOutcome::Complete {
                    status: status.as_u16(),
                    body_bytes: body.len(),
                },
                Err(stage) => ExchangeOutcome::Failed(stage),
            };
            Err(io::Error::other(RequestFailure { exchange, driver }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_response_cannot_hide_a_failed_driver_or_leak_body() {
        let error = settle(
            Ok((StatusCode::OK, Bytes::from_static(b"private-value"))),
            Some(DriverFailure {
                during_shutdown: false,
                io_kind: Some(io::ErrorKind::ConnectionReset),
            }),
        )
        .unwrap_err();
        let detail = error
            .get_ref()
            .unwrap()
            .downcast_ref::<RequestFailure>()
            .unwrap();
        assert_eq!(
            detail.exchange,
            ExchangeOutcome::Complete {
                status: 200,
                body_bytes: 13
            }
        );
        assert_eq!(
            detail.driver,
            Some(DriverFailure {
                during_shutdown: false,
                io_kind: Some(io::ErrorKind::ConnectionReset)
            })
        );
        assert!(!format!("{error:?} {error}").contains("private-value"));
    }

    #[test]
    fn both_failures_survive_and_clean_driver_does_not_hide_exchange_failure() {
        for stage in [ExchangeFailure::Request, ExchangeFailure::Body] {
            for driver in [
                None,
                Some(DriverFailure {
                    during_shutdown: false,
                    io_kind: None,
                }),
            ] {
                let failed_driver = driver.is_some();
                let error = settle(Err(stage), driver).unwrap_err();
                let detail = error
                    .get_ref()
                    .unwrap()
                    .downcast_ref::<RequestFailure>()
                    .unwrap();
                assert_eq!(detail.exchange, ExchangeOutcome::Failed(stage));
                assert_eq!(detail.driver.is_some(), failed_driver);
            }
        }
        assert_eq!(
            settle(Ok((StatusCode::OK, Bytes::new())), None).unwrap().0,
            StatusCode::OK
        );
    }
}
