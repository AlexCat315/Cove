//! The `http`/`https` URL subset the client sends.
use crate::HttpError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Url {
    pub secure: bool,
    pub host: String,
    pub port: u16,
    /// Path plus query, always starting with `/`; never the fragment.
    pub path: String,
    /// `host[:port]` exactly as written, for the `Host` header.
    pub authority: String,
}

impl Url {
    /// Parses an `http`/`https` URL. Rejects other schemes, userinfo, a
    /// missing host and an invalid port.
    pub fn parse(url: &str) -> Result<Self, HttpError> {
        let unsupported = || HttpError::UnsupportedUrl(url.chars().take(256).collect());
        if url.len() > 8192 || url.bytes().any(|byte| byte <= b' ' || byte == 0x7f) {
            return Err(unsupported());
        }
        let (scheme, rest) = url.split_once("://").ok_or_else(unsupported)?;
        let secure = match scheme.to_ascii_lowercase().as_str() {
            "https" => true,
            "http" => false,
            _ => return Err(unsupported()),
        };
        let rest = rest.split('#').next().unwrap_or(rest);
        let split = rest.find(['/', '?']).unwrap_or(rest.len());
        let (authority, target) = rest.split_at(split);
        if authority.contains('@') {
            return Err(unsupported());
        }
        let (host, port) = if let Some(literal) = authority.strip_prefix('[') {
            let (host, after) = literal.split_once(']').ok_or_else(unsupported)?;
            let port = match after.strip_prefix(':') {
                Some(port) => port.parse::<u16>().map_err(|_| unsupported())?,
                None if after.is_empty() => default_port(secure),
                None => return Err(unsupported()),
            };
            (host, port)
        } else {
            match authority.rsplit_once(':') {
                Some((host, port)) => (host, port.parse::<u16>().map_err(|_| unsupported())?),
                None => (authority, default_port(secure)),
            }
        };
        if host.is_empty() || port == 0 {
            return Err(unsupported());
        }
        let path = if target.starts_with('/') {
            target.to_owned()
        } else {
            format!("/{target}")
        };
        Ok(Self {
            secure,
            host: host.to_ascii_lowercase(),
            port,
            path,
            authority: authority.to_owned(),
        })
    }

    pub fn scheme(&self) -> &'static str {
        if self.secure { "https" } else { "http" }
    }

    /// `true` when the host is an IPv4 or IPv6 literal (no SNI is sent).
    pub fn is_ip_literal(&self) -> bool {
        self.host.parse::<std::net::IpAddr>().is_ok()
    }

    pub fn same_origin(&self, other: &Url) -> bool {
        self.secure == other.secure && self.host == other.host && self.port == other.port
    }

    /// Resolves a `Location` header against this URL.
    pub fn join(&self, location: &str) -> Result<Url, HttpError> {
        if location.contains("://") {
            return Url::parse(location);
        }
        if let Some(rest) = location.strip_prefix("//") {
            return Url::parse(&format!("{}://{rest}", self.scheme()));
        }
        let path = if location.starts_with('/') {
            location.to_owned()
        } else if let Some(query) = location.strip_prefix('?') {
            let base = self.path.split('?').next().unwrap_or("/");
            format!("{base}?{query}")
        } else {
            let base = self.path.split('?').next().unwrap_or("/");
            let directory = &base[..base.rfind('/').map_or(1, |index| index + 1)];
            format!("{directory}{location}")
        };
        Url::parse(&format!("{}://{}{}", self.scheme(), self.authority, path))
    }
}

impl std::fmt::Display for Url {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}://{}{}",
            self.scheme(),
            self.authority,
            self.path
        )
    }
}

fn default_port(secure: bool) -> u16 {
    if secure { 443 } else { 80 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ports_paths_and_literals() {
        let url = Url::parse("https://Example.com:8443/a/b?c=d#frag").unwrap();
        assert!(url.secure);
        assert_eq!(
            (url.host.as_str(), url.port, url.path.as_str()),
            ("example.com", 8443, "/a/b?c=d")
        );
        let url = Url::parse("http://example.com?x=1").unwrap();
        assert_eq!((url.port, url.path.as_str()), (80, "/?x=1"));
        let url = Url::parse("http://[::1]:9000/").unwrap();
        assert_eq!((url.host.as_str(), url.port), ("::1", 9000));
        assert!(url.is_ip_literal());
        assert_eq!(Url::parse("https://[::1]").unwrap().port, 443);
    }

    #[test]
    fn rejects_unsupported_urls() {
        for url in [
            "ftp://example.com",
            "http://user@example.com/",
            "http://:80/",
            "http://example.com:0/",
            "http://example.com:x/",
            "http://exa mple.com/",
            "http://example.com/\r\nX: y",
        ] {
            assert!(Url::parse(url).is_err(), "{url}");
        }
    }

    #[test]
    fn joins_redirect_locations() {
        let base = Url::parse("https://a.test:444/dir/page?q=1").unwrap();
        assert_eq!(base.join("/x").unwrap().to_string(), "https://a.test:444/x");
        assert_eq!(
            base.join("next").unwrap().to_string(),
            "https://a.test:444/dir/next"
        );
        assert_eq!(
            base.join("?q=2").unwrap().to_string(),
            "https://a.test:444/dir/page?q=2"
        );
        assert_eq!(
            base.join("//b.test/y").unwrap().to_string(),
            "https://b.test/y"
        );
        assert_eq!(
            base.join("http://c.test/").unwrap().to_string(),
            "http://c.test/"
        );
    }
}
