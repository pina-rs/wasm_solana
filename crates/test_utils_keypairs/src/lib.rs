use solana_keypair::Keypair;

/// `tBuug63EhqE836n1dirg3sZ5KwZuG6P6i5nDHYrryyX`
pub const SECRET_KEY_TREASURY: &[u8] = &[
	221, 217, 170, 53, 99, 128, 242, 129, 252, 218, 171, 240, 150, 97, 116, 163, 179, 56, 130, 14,
	116, 6, 138, 101, 20, 56, 204, 21, 15, 55, 23, 233, 13, 28, 245, 177, 96, 233, 242, 131, 42,
	68, 123, 66, 30, 135, 233, 223, 228, 97, 147, 147, 153, 159, 172, 75, 99, 57, 153, 150, 207,
	92, 79, 198,
];

/// `EjxoynXa5jTq4MyZzWtTz6Uq8UHQDxebkWhvEkDXoZxH`
pub const SECRET_KEY_AUTHORITY: &[u8] = &[
	101, 184, 219, 23, 35, 211, 62, 206, 159, 197, 60, 28, 139, 89, 6, 38, 92, 125, 41, 170, 83,
	200, 129, 208, 118, 215, 237, 167, 86, 86, 9, 83, 204, 41, 109, 182, 252, 170, 242, 95, 90,
	190, 103, 20, 66, 112, 0, 95, 11, 33, 201, 29, 129, 252, 77, 105, 38, 75, 95, 65, 23, 191, 53,
	214,
];

/// Account for the admin address during testing. Add this to your .env file
///
/// ```env
/// ADMIN_PUBKEY = "4z5X2suocz9szaQnSshj2AW8tuLgUVmYUxiW9hhPaRHs"
/// ```
pub const SECRET_KEY_ADMIN: &[u8] = &[
	72, 88, 187, 16, 91, 210, 34, 156, 133, 215, 12, 132, 105, 166, 75, 105, 141, 144, 240, 12, 96,
	68, 95, 210, 9, 13, 89, 18, 157, 241, 77, 79, 59, 50, 70, 114, 194, 140, 21, 107, 154, 91, 109,
	48, 69, 230, 233, 147, 121, 226, 53, 10, 104, 221, 52, 9, 184, 42, 203, 244, 178, 241, 55, 186,
];

/// `B6FAryxc6pfLuK4to1BuvhqVJQm2V1cthguAmSPvqPug`
pub const SECRET_KEY_WALLET: &[u8] = &[
	122, 234, 133, 232, 80, 195, 2, 115, 237, 183, 24, 30, 85, 198, 199, 101, 125, 18, 35, 185,
	237, 150, 219, 78, 22, 118, 140, 56, 55, 118, 119, 180, 149, 236, 203, 160, 52, 95, 187, 57,
	214, 47, 184, 28, 44, 195, 225, 83, 138, 219, 119, 32, 100, 18, 255, 236, 227, 160, 9, 12, 182,
	174, 75, 215,
];

pub fn get_admin_keypair() -> Keypair {
	Keypair::try_from(SECRET_KEY_ADMIN).unwrap()
}

pub fn get_authority_keypair() -> Keypair {
	Keypair::try_from(SECRET_KEY_AUTHORITY).unwrap()
}

pub fn get_treasury_keypair() -> Keypair {
	Keypair::try_from(SECRET_KEY_TREASURY).unwrap()
}

pub fn get_wallet_keypair() -> Keypair {
	Keypair::try_from(SECRET_KEY_WALLET).unwrap()
}
