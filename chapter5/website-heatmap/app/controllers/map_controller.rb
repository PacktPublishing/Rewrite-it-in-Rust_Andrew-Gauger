class MapController < ApplicationController
  SCENARIOS = %w[cluster highway].freeze
  ENGINES = %w[legacy wasm].freeze

  def index
    @scenario = SCENARIOS.include?(params[:scenario]) ? params[:scenario] : "cluster"
    @engine = ENGINES.include?(params[:engine]) ? params[:engine] : Rails.application.config.x.map_engine
  end

  def readings
    scenario = SCENARIOS.include?(params[:scenario]) ? params[:scenario] : "cluster"
    render json: SensorDataGenerator.new(scenario).readings
  end
end
