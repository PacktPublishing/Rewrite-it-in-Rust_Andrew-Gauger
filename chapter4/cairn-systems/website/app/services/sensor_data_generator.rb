require "digest"

# Produces raw, per-minute GPS/temperature/vibration readings for two demo
# fleets: a stationary cluster of farmland sensors, and a set of
# refrigerated containers moving along a highway route. Every reading is
# returned individually (not aggregated per-sensor) on purpose -- this is
# the naive dataset the legacy map view plots one marker per point.
class SensorDataGenerator
  SENSOR_COUNT = 20
  READINGS_PER_SENSOR = 360 # 6 hours at 1 reading/minute

  CLUSTER_CENTER = { lat: 41.0500, lng: -98.3450 }.freeze # farmland near Grand Island, NE
  CLUSTER_RADIUS_METERS = 350

  # A stretch of I-80 through Nebraska, Lincoln -> North Platte.
  HIGHWAY_WAYPOINTS = [
    { lat: 40.8136, lng: -96.7026 },
    { lat: 40.8683, lng: -97.5928 },
    { lat: 40.9264, lng: -98.3420 },
    { lat: 40.6994, lng: -99.0817 },
    { lat: 41.1239, lng: -100.7654 }
  ].freeze

  def initialize(scenario)
    @scenario = scenario.to_s
  end

  def readings
    @scenario == "highway" ? highway_readings : cluster_readings
  end

  private

  def cluster_readings
    start_time = Time.current - READINGS_PER_SENSOR.minutes

    (1..SENSOR_COUNT).flat_map do |n|
      sensor_id = "FIELD-%02d" % n
      rng = Random.new(seed_for(sensor_id))
      base_lat, base_lng = jittered_point(CLUSTER_CENTER[:lat], CLUSTER_CENTER[:lng], CLUSTER_RADIUS_METERS, rng)
      baseline_temp = 62 + rng.rand(0..12)

      READINGS_PER_SENSOR.times.map do |i|
        timestamp = start_time + i.minutes
        hour_of_day = timestamp.hour + timestamp.min / 60.0
        daily_curve = Math.sin((hour_of_day - 6) / 24.0 * 2 * Math::PI) * 12
        lat, lng = jittered_point(base_lat, base_lng, 4, rng)
        idle_spike = rng.rand < 0.02 ? rng.rand(0.3..0.9) : 0.0

        {
          sensor_id: sensor_id,
          scenario: "cluster",
          lat: lat.round(6),
          lng: lng.round(6),
          temperature_f: (baseline_temp + daily_curve + rng.rand(-1.5..1.5)).round(1),
          vibration_g: (rng.rand(0.0..0.05) + idle_spike).round(3),
          recorded_at: timestamp.iso8601
        }
      end
    end
  end

  def highway_readings
    start_time = Time.current - READINGS_PER_SENSOR.minutes
    total_length = route_length(HIGHWAY_WAYPOINTS)

    (1..SENSOR_COUNT).flat_map do |n|
      sensor_id = "CTR-%03d" % (100 + n)
      rng = Random.new(seed_for(sensor_id))
      start_offset = rng.rand(0...total_length)
      speed_mps = (55 + rng.rand(0..15)) * 0.44704
      reefer_setpoint_c = rng.rand(-2..4)

      READINGS_PER_SENSOR.times.map do |i|
        timestamp = start_time + i.minutes
        distance = (start_offset + speed_mps * i * 60) % total_length
        lat, lng = point_at_distance(HIGHWAY_WAYPOINTS, distance)
        lat, lng = jittered_point(lat, lng, 5, rng)
        pothole_spike = rng.rand < 0.05 ? rng.rand(1.2..2.5) : 0.0

        {
          sensor_id: sensor_id,
          scenario: "highway",
          lat: lat.round(6),
          lng: lng.round(6),
          temperature_f: (reefer_setpoint_c * 9.0 / 5 + 32 + rng.rand(-0.8..0.8)).round(1),
          vibration_g: (0.25 + rng.rand(0.0..0.35) + pothole_spike).round(3),
          recorded_at: timestamp.iso8601
        }
      end
    end
  end

  def seed_for(sensor_id)
    Digest::MD5.hexdigest("#{@scenario}-#{sensor_id}").to_i(16) % (2**32)
  end

  def jittered_point(lat, lng, radius_meters, rng)
    return [lat, lng] if radius_meters.zero?

    r = radius_meters * Math.sqrt(rng.rand)
    theta = rng.rand * 2 * Math::PI
    dlat = (r * Math.cos(theta)) / 111_320.0
    dlng = (r * Math.sin(theta)) / (111_320.0 * Math.cos(lat * Math::PI / 180))
    [lat + dlat, lng + dlng]
  end

  def haversine_meters(a, b)
    r = 6_371_000.0
    lat1, lat2 = a[:lat] * Math::PI / 180, b[:lat] * Math::PI / 180
    dlat = lat2 - lat1
    dlng = (b[:lng] - a[:lng]) * Math::PI / 180
    h = Math.sin(dlat / 2)**2 + Math.cos(lat1) * Math.cos(lat2) * Math.sin(dlng / 2)**2
    2 * r * Math.asin(Math.sqrt(h))
  end

  def route_length(waypoints)
    waypoints.each_cons(2).sum { |a, b| haversine_meters(a, b) }
  end

  def point_at_distance(waypoints, distance)
    remaining = distance
    waypoints.each_cons(2) do |a, b|
      segment = haversine_meters(a, b)
      if remaining <= segment
        fraction = segment.zero? ? 0 : remaining / segment
        return [a[:lat] + (b[:lat] - a[:lat]) * fraction, a[:lng] + (b[:lng] - a[:lng]) * fraction]
      end
      remaining -= segment
    end
    last = waypoints.last
    [last[:lat], last[:lng]]
  end
end
