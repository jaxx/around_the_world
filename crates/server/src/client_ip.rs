//! The address a request really came from, for the per-address rate limit.
//!
//! Behind a host's proxies the TCP peer is the last proxy. Each proxy
//! appends the address it received the request from to `X-Forwarded-For`,
//! but a client can put anything it likes at the front. So with `hops`
//! trusted proxies, the client is the `hops`-th entry from the right.

use std::net::IpAddr;

/// The client's address. `hops` is the number of trusted proxies in front
/// of the server (`TRUST_PROXY_HOPS`); 0 trusts no header at all.
pub fn client_ip(forwarded_for: Option<&str>, peer: IpAddr, hops: u8) -> IpAddr {
    if hops == 0 {
        return peer;
    }
    let entries: Vec<&str> = forwarded_for
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .collect();
    if entries.is_empty() {
        return peer;
    }
    // Fewer entries than trusted hops means a misconfiguration; the first
    // entry is the best guess then.
    let index = entries.len().saturating_sub(hops as usize);
    entries[index].parse().unwrap_or(peer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    const PEER: &str = "10.0.0.9";

    #[test]
    fn without_trusted_proxies_the_header_is_ignored() {
        assert_eq!(client_ip(Some("6.6.6.6"), ip(PEER), 0), ip(PEER));
    }

    #[test]
    fn the_entry_added_by_the_outermost_trusted_proxy_is_the_client() {
        let header = Some("6.6.6.6, 1.2.3.4, 10.0.0.2");
        assert_eq!(client_ip(header, ip(PEER), 1), ip("10.0.0.2"));
        assert_eq!(client_ip(header, ip(PEER), 2), ip("1.2.3.4"));
    }

    #[test]
    fn a_forged_front_entry_is_not_trusted() {
        // The client sent "6.6.6.6"; two proxies appended what they saw.
        assert_eq!(
            client_ip(Some("6.6.6.6, 1.2.3.4, 10.0.0.2"), ip(PEER), 2),
            ip("1.2.3.4")
        );
    }

    #[test]
    fn fewer_entries_than_hops_falls_back_to_the_first() {
        assert_eq!(client_ip(Some("1.2.3.4"), ip(PEER), 3), ip("1.2.3.4"));
    }

    #[test]
    fn a_missing_or_unreadable_header_falls_back_to_the_peer() {
        assert_eq!(client_ip(None, ip(PEER), 2), ip(PEER));
        assert_eq!(client_ip(Some(""), ip(PEER), 2), ip(PEER));
        assert_eq!(client_ip(Some("nonsense, 1.2.3.4"), ip(PEER), 2), ip(PEER));
    }

    #[test]
    fn spaces_and_ipv6_are_fine() {
        assert_eq!(
            client_ip(Some(" 2001:db8::1 ,10.0.0.2"), ip(PEER), 2),
            ip("2001:db8::1")
        );
    }
}
