/// Assert that the banks client simulation errors with the provided
/// error.
#[macro_export]
macro_rules! assert_banks_client_simulated_error {
	($result:ident, $expected_error:path) => {{
		let error_name = $expected_error.name();

		$crate::__private::check!(
			$result.result.unwrap().is_err(),
			"the simulation was expected to error"
		);
		$crate::__private::check!(
			$result
				.simulation_details
				.unwrap()
				.logs
				.iter()
				.any(|log| log.contains(format!("Error Code: {error_name}").as_str())),
			"`{:#?}` not found in logs",
			$expected_error,
		);
	}};
}

/// Assert that the banks client errors with the expected error code.
#[macro_export]
macro_rules! assert_banks_client_metadata_error {
	($result:ident, $expected_error:path) => {{
		let error_name = $expected_error.name();

		$crate::__private::check!(
			$result.result.is_err(),
			"the simulation was expected to error"
		);
		$crate::__private::check!(
			$result
				.metadata
				.unwrap()
				.log_messages
				.iter()
				.any(|log| log.contains(format!("Error Code: {error_name}").as_str())),
			"`{:#?}` not found in logs",
			$expected_error,
		);
	}};
}
