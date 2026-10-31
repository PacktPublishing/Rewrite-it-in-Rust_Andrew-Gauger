class MapController < ApplicationController
  SCENARIOS = %w[cluster highway].freeze

  def index
    @scenario = SCENARIOS.include?(params[:scenario]) ? params[:scenario] : "cluster"
    @readings = SensorDataGenerator.new(@scenario).readings
  end
end
