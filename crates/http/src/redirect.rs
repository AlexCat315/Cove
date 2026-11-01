//! Redirect policy for backends that follow redirects themselves.
#![cfg_attr(not(any(cove_http_socket, windows)), allow(dead_code))]

use crate::{Headers, HttpError, Url};

pub struct Follow<'a> {
    pub method: &'a mut String,
    pub headers: &'a mut Headers,
    pub body: &'a mut Vec<u8>,
    pub redirects: &'a mut usize,
}

/// `Ok(Some(next))` when the response should be followed, `Ok(None)` when it
/// is the final response.
pub fn next(
    status: u16,
    location: Option<&str>,
    url: &Url,
    max_redirects: usize,
    follow: Follow<'_>,
) -> Result<Option<Url>, HttpError> {
    let Some(location) = location.filter(|_| matches!(status, 301 | 302 | 303 | 307 | 308)) else {
        return Ok(None);
    };
    if max_redirects == 0 {
        return Ok(None);
    }
    *follow.redirects += 1;
    if *follow.redirects > max_redirects {
        return Err(HttpError::TooManyRedirects);
    }
    let next = url.join(location)?;
    if url.secure && !next.secure {
        return Err(HttpError::Network(format!(
            "refusing redirect from https to {next}"
        )));
    }
    // 303, and 301/302 after POST, continue as a body-less GET (as browsers and
    // NSURLSession do); 307/308 repeat the request unchanged.
    if status == 303 && follow.method != "HEAD"
        || matches!(status, 301 | 302) && follow.method == "POST"
    {
        *follow.method = "GET".into();
        follow.body.clear();
        follow.headers.remove("content-type");
    }
    if !url.same_origin(&next) {
        for name in ["authorization", "cookie", "proxy-authorization"] {
            follow.headers.remove(name);
        }
    }
    Ok(Some(next))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(
        status: u16,
        method: &str,
        from: &str,
        to: &str,
    ) -> (Result<Option<Url>, HttpError>, String, Headers, Vec<u8>) {
        let mut method = method.to_owned();
        let mut headers: Headers = [("Authorization", "x"), ("Content-Type", "a/b")]
            .into_iter()
            .collect();
        let mut body = b"data".to_vec();
        let mut redirects = 0;
        let result = next(
            status,
            Some(to),
            &Url::parse(from).unwrap(),
            5,
            Follow {
                method: &mut method,
                headers: &mut headers,
                body: &mut body,
                redirects: &mut redirects,
            },
        );
        (result, method, headers, body)
    }

    #[test]
    fn rewrites_methods_like_browsers() {
        let (next, method, headers, body) = run(303, "PUT", "http://a.test/", "/b");
        assert_eq!(next.unwrap().unwrap().to_string(), "http://a.test/b");
        assert_eq!(
            (
                method.as_str(),
                body.is_empty(),
                headers.contains("content-type")
            ),
            ("GET", true, false)
        );
        let (_, method, _, body) = run(307, "POST", "http://a.test/", "/b");
        assert_eq!((method.as_str(), body.as_slice()), ("POST", &b"data"[..]));
        let (_, method, _, _) = run(302, "POST", "http://a.test/", "/b");
        assert_eq!(method, "GET");
    }

    #[test]
    fn strips_credentials_across_origins_and_refuses_downgrades() {
        let (_, _, headers, _) = run(302, "GET", "http://a.test/", "http://b.test/");
        assert!(!headers.contains("authorization"));
        let (_, _, headers, _) = run(302, "GET", "http://a.test/", "/same");
        assert!(headers.contains("authorization"));
        assert!(
            run(302, "GET", "https://a.test/", "http://a.test/")
                .0
                .is_err()
        );
        assert!(run(200, "GET", "http://a.test/", "/x").0.unwrap().is_none());
    }

    #[test]
    fn enforces_the_redirect_budget() {
        let url = Url::parse("http://a.test/").unwrap();
        let (mut method, mut headers, mut body, mut redirects) =
            ("GET".to_owned(), Headers::new(), Vec::new(), 1);
        let follow = Follow {
            method: &mut method,
            headers: &mut headers,
            body: &mut body,
            redirects: &mut redirects,
        };
        assert_eq!(
            next(302, Some("/"), &url, 1, follow),
            Err(HttpError::TooManyRedirects)
        );
        let follow = Follow {
            method: &mut method,
            headers: &mut headers,
            body: &mut body,
            redirects: &mut redirects,
        };
        assert_eq!(next(302, Some("/"), &url, 0, follow), Ok(None));
    }
}
