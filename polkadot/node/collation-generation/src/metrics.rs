// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

use polkadot_node_subsystem_util::metrics::{self, prometheus};

#[derive(Clone)]
pub(crate) struct MetricsInner {
	pub(crate) collations_generated_total: prometheus::Counter<prometheus::U64>,
	pub(crate) new_activation: prometheus::Histogram,
	pub(crate) submit_collation: prometheus::Histogram,
}

/// `CollationGenerationSubsystem` metrics.
#[derive(Default, Clone)]
pub struct Metrics(pub(crate) Option<MetricsInner>);

impl Metrics {
	pub fn on_collation_generated(&self) {
		if let Some(metrics) = &self.0 {
			metrics.collations_generated_total.inc();
		}
	}

	/// Provide a timer for new activations which updates on drop.
	pub fn time_new_activation(&self) -> Option<metrics::prometheus::prometheus::HistogramTimer> {
		self.0.as_ref().map(|metrics| metrics.new_activation.start_timer())
	}

	/// Provide a timer for submitting a collation which updates on drop.
	pub fn time_submit_collation(&self) -> Option<metrics::prometheus::prometheus::HistogramTimer> {
		self.0.as_ref().map(|metrics| metrics.submit_collation.start_timer())
	}
}

impl metrics::Metrics for Metrics {
	fn try_register(registry: &prometheus::Registry) -> Result<Self, prometheus::PrometheusError> {
		let metrics = MetricsInner {
			collations_generated_total: prometheus::register(
				prometheus::Counter::new(
					"polkadot_parachain_collations_generated_total",
					"Number of collations generated.",
				)?,
				registry,
			)?,
			new_activation: prometheus::register(
				prometheus::Histogram::with_opts(prometheus::HistogramOpts::new(
					"polkadot_parachain_collation_generation_new_activations",
					"Time spent within fn handle_new_activation",
				))?,
				registry,
			)?,
			submit_collation: prometheus::register(
				prometheus::Histogram::with_opts(prometheus::HistogramOpts::new(
					"polkadot_parachain_collation_generation_submit_collation",
					"Time spent preparing and submitting a collation to the network protocol",
				))?,
				registry,
			)?,
		};
		Ok(Metrics(Some(metrics)))
	}
}
