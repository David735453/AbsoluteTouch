# frozen_string_literal: true

require "minitest/autorun"

require_relative "build_number"

# The build number must increase for every upload, whatever the run number is.
class BuildNumberTest < Minitest::Test
  def test_uses_the_run_number_when_it_is_higher
    assert_equal(183, BuildNumber.resolve(floor: 183, latest: 12))
  end

  def test_moves_past_the_newest_upload
    assert_equal(184, BuildNumber.resolve(floor: 12, latest: 183))
  end

  def test_first_upload_uses_the_run_number
    assert_equal(7, BuildNumber.resolve(floor: 7, latest: 0))
  end

  def test_unreadable_testflight_falls_back_to_the_run_number
    assert_equal(7, BuildNumber.resolve(floor: 7, latest: nil))
  end

  def test_never_returns_a_number_below_one
    assert_equal(1, BuildNumber.resolve(floor: 0, latest: nil))
    assert_equal(1, BuildNumber.resolve(floor: -4, latest: -9))
  end

  def test_accepts_numbers_from_the_environment
    assert_equal(12, BuildNumber.resolve(floor: "12", latest: "5"))
  end
end
