//! SO_GETHOSTID returns IPv4 bits through a signed IOS result register.
//!
//! Unicast addresses from 128.0.0.0 through 223.255.255.255 have the sign bit
//! set. Do not interpret those addresses as errors. Negative error codes and
//! reserved multicast/class-E values remain errors; zero means DHCP is pending.

fn checked_host_id(value: i32) -> Result<i32, i32> {
    if value < 0 && (value as u32 >> 24) >= 224 {
        Err(value)
    } else {
        Ok(value)
    }
}

pub(super) fn active_attach_ip(
    host_id: i32,
    timeout_error: i32,
    startup: impl FnOnce() -> Result<i32, i32>,
) -> Result<i32, i32> {
    let host_id = checked_host_id(host_id)?;
    let host_id = if host_id == 0 { startup()? } else { host_id };
    match host_id {
        0 => Err(timeout_error),
        value => checked_host_id(value),
    }
}

pub(super) const MAX_NETWORK_IP_RETRIES: usize = 100;

pub(super) fn retry_network_ip(
    timeout_error: i32,
    mut query: impl FnMut() -> Result<i32, i32>,
    mut wait: impl FnMut(),
) -> Result<i32, i32> {
    for attempt in 0..=MAX_NETWORK_IP_RETRIES {
        let host_id = checked_host_id(query()?)?;
        if host_id != 0 {
            return Ok(host_id);
        }
        if attempt < MAX_NETWORK_IP_RETRIES {
            wait();
        }
    }
    Err(timeout_error)
}

pub(super) fn startup_network_ip(
    timeout_error: i32,
    mut startup: impl FnMut() -> Result<i32, i32>,
    mut query: impl FnMut() -> Result<i32, i32>,
    wait: impl FnMut(),
) -> Result<i32, i32> {
    retry_network_ip(
        timeout_error,
        || {
            let result = startup()?;
            if result < 0 {
                return Err(result);
            }
            query()
        },
        wait,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_attach_accepts_signed_unicast_addresses_without_restarting_ios() {
        for address in [
            0x0a00_00f2u32,
            0x8000_0001,
            0xac10_0002,
            0xc0a8_0203,
            0xdfff_fffe,
        ] {
            assert_eq!(
                active_attach_ip(address as i32, -110, || panic!("unneeded startup")),
                Ok(address as i32)
            );
        }
    }

    #[test]
    fn dhcp_can_assign_192_168_after_zero_host_ids() {
        let mut queries = 0;
        let mut startups = 0;
        let mut waits = 0;
        let address = 0xc0a8_0203u32 as i32;
        assert_eq!(
            active_attach_ip(0, -110, || startup_network_ip(
                -110,
                || {
                    startups += 1;
                    Ok(0)
                },
                || {
                    queries += 1;
                    Ok(if queries < 3 { 0 } else { address })
                },
                || waits += 1,
            )),
            Ok(address)
        );
        assert_eq!((queries, startups, waits), (3, 3, 2));
    }

    #[test]
    fn negative_startup_and_query_errors_remain_failures() {
        for error in [-1, -4, -6, -110, -10000] {
            assert_eq!(
                active_attach_ip(error, -110, || panic!("startup after error")),
                Err(error)
            );
            assert_eq!(active_attach_ip(0, -110, || Ok(error)), Err(error));
            assert_eq!(
                startup_network_ip(-110, || Ok(error), || panic!("query after error"), || {}),
                Err(error)
            );
            assert_eq!(retry_network_ip(-110, || Err(error), || {}), Err(error));
        }
    }

    #[test]
    fn zero_address_still_times_out_with_the_existing_retry_bound() {
        let mut queries = 0;
        let mut waits = 0;
        assert_eq!(
            retry_network_ip(
                -110,
                || {
                    queries += 1;
                    Ok(0)
                },
                || waits += 1
            ),
            Err(-110)
        );
        assert_eq!(queries, 101);
        assert_eq!(waits, 100);
        assert_eq!(active_attach_ip(0, -110, || Ok(0)), Err(-110));
    }
}
