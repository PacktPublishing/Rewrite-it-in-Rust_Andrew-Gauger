#!/usr/bin/env ruby
# frozen_string_literal: true

# Runs the website-heatmap app's real SensorDataGenerator without booting
# Rails. That service only leans on a few ActiveSupport niceties
# (`Time.current`, `Integer#minutes`, `Time#iso8601`); stub those and the
# generator itself is plain Ruby. Output is byte-for-byte what
# `GET /readings.json?scenario=<name>` serves, so the benchmark runs on the
# real dataset, not a re-derived approximation.
#
# Usage:
#   ruby gen-readings.rb cluster > fixtures/readings.cluster.json
#   ruby gen-readings.rb highway > fixtures/readings.highway.json

require "json"
require "time"

# --- minimal ActiveSupport shims -------------------------------------------
class Numeric
  def minutes
    self * 60
  end
  alias minute minutes
end

class Time
  def self.current
    now
  end
end
# `Time#iso8601` comes from stdlib `time` (required above). `Time + Numeric`
# and `Time - Numeric` are already core Ruby (seconds).
# -------------------------------------------------------------------------

GENERATOR = File.expand_path(
  "../website-heatmap/app/services/sensor_data_generator.rb", __dir__
)
require GENERATOR

scenario = ARGV[0] || "cluster"
abort "scenario must be 'cluster' or 'highway'" unless %w[cluster highway].include?(scenario)

STDOUT.write(JSON.generate(SensorDataGenerator.new(scenario).readings))
