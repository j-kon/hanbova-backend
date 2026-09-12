use std::net::{IpAddr, Ipv4Addr};

/// Static configuration check, not DNS resolution or redirect/egress control.
pub(super) fn is_hosted_https_url(value: &str) -> bool {
    if value
        .chars()
        .any(|ch| ch.is_whitespace() || ch.is_control() || ch == '\\')
    {
        return false;
    }
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.port() == Some(0)
    {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let host = host.strip_suffix('.').unwrap_or(host);
    if let Ok(address) = host.parse::<IpAddr>() {
        return match address {
            IpAddr::V4(address) => public_v4(address),
            IpAddr::V6(address) => match address.to_ipv4_mapped() {
                Some(address) => public_v4(address),
                None => address.octets()[0] & 0xe0 == 0x20,
            },
        };
    }
    if !host.contains('.')
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
        || host.len() > 253
    {
        return false;
    }
    host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    })
}

fn public_v4(address: Ipv4Addr) -> bool {
    let b = address.octets();
    !(address.is_private()
        || address.is_loopback()
        || address.is_link_local()
        || b[0] == 0
        || b[0] >= 224
        || (b[0] == 100 && (64..=127).contains(&b[1]))
        || (b[0] == 198 && (b[1] == 18 || b[1] == 19)))
}
