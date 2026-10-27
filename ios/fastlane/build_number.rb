# frozen_string_literal: true

# Chooses the `CURRENT_PROJECT_VERSION` for one upload.
#
# Kept free of fastlane so it can be tested with plain Ruby: the Fastfile does
# the App Store Connect query and passes the answer in.
module BuildNumber
  # Number to upload with.
  #
  # - Parameter `floor`: Smallest acceptable number, usually the workflow run
  #   number.
  # - Parameter `latest`: Newest build TestFlight has for the version, or `nil`
  #   when it could not be read.
  # - Returns: One past `latest`, or `floor` when that is higher; never below 1.
  def self.resolve(floor:, latest: nil)
    number = [floor.to_i, 1].max
    return number if latest.nil?

    [number, latest.to_i + 1].max
  end
end
