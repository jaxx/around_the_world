//! Settings from the environment.

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// `PORT`, default 8080.
    pub port: u16,
    /// `STATIC_DIR`: the built browser client, if this server serves it.
    pub static_dir: Option<PathBuf>,
    /// `TRUST_PROXY_HOPS`: how many proxies in front of the server append to
    /// `X-Forwarded-For`; default 0 trusts none (see [`crate::client_ip`]).
    pub proxy_hops: u8,
}

impl Config {
    /// Reads the settings through `var`, e.g. `|name| std::env::var(name).ok()`.
    pub fn from_env(var: impl Fn(&str) -> Option<String>) -> Result<Config, String> {
        let port = match var("PORT") {
            None => 8080,
            Some(port) => port
                .trim()
                .parse()
                .map_err(|_| format!("PORT must be a port number, got {port:?}"))?,
        };
        let static_dir = var("STATIC_DIR")
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from);
        let proxy_hops = match var("TRUST_PROXY_HOPS") {
            None => 0,
            Some(hops) => hops
                .trim()
                .parse()
                .map_err(|_| format!("TRUST_PROXY_HOPS must be a small number, got {hops:?}"))?,
        };
        Ok(Config {
            port,
            static_dir,
            proxy_hops,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone())
        }
    }

    #[test]
    fn defaults_to_port_8080_and_no_static_files() {
        assert_eq!(
            Config::from_env(env(&[])),
            Ok(Config {
                port: 8080,
                static_dir: None,
                proxy_hops: 0
            })
        );
    }

    #[test]
    fn reads_port_and_static_dir() {
        assert_eq!(
            Config::from_env(env(&[("PORT", "3000"), ("STATIC_DIR", "dist")])),
            Ok(Config {
                port: 3000,
                static_dir: Some(PathBuf::from("dist")),
                proxy_hops: 0
            })
        );
    }

    #[test]
    fn a_bad_port_is_an_error() {
        let err = Config::from_env(env(&[("PORT", "eighty")])).unwrap_err();
        assert!(err.contains("PORT"), "{err}");
    }

    #[test]
    fn an_empty_static_dir_means_none() {
        assert_eq!(
            Config::from_env(env(&[("STATIC_DIR", "")]))
                .unwrap()
                .static_dir,
            None
        );
    }

    #[test]
    fn reads_the_trusted_proxy_hops() {
        let config = Config::from_env(env(&[("TRUST_PROXY_HOPS", "3")])).unwrap();
        assert_eq!(config.proxy_hops, 3);
    }

    #[test]
    fn bad_proxy_hops_are_an_error() {
        let err = Config::from_env(env(&[("TRUST_PROXY_HOPS", "many")])).unwrap_err();
        assert!(err.contains("TRUST_PROXY_HOPS"), "{err}");
    }
}
