use crate::constants::TIER_INFO;

pub fn get_tier(amount: u64) -> u8 {
  for (i, x) in TIER_INFO.iter().enumerate() {
    if amount < *x {
      return i as u8;
    }
  }

  return TIER_INFO.len() as u8;
}
