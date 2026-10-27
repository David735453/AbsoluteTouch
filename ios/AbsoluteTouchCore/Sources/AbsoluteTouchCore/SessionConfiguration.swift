import Foundation

/// Why a streaming session stopped.
public enum SessionFailure: Error, Sendable, Equatable {
	/// The network transport failed or closed unexpectedly.
	case transport(String)
}

extension SessionFailure: LocalizedError {
	/// A human-readable description of the failure.
	public var errorDescription: String? {
		switch self {
		case .transport(let message):
			message
		}
	}
}

/// State of the iPad sender.
public enum SessionState: Sendable, Equatable {
	/// No session is running.
	case idle

	/// `HELLO` is being repeated until the desktop acknowledges it.
	///
	/// The associated value counts the attempts made in this handshake.
	case connecting(attempt: Int)

	/// The desktop accepted the session.
	case streaming(sessionID: UInt32, hostCapabilities: PacketFlags)

	/// The transport failed. The app must start a new session.
	case failed(SessionFailure)
}

extension SessionState {
	/// Whether the session is established.
	public var isStreaming: Bool {
		if case .streaming = self {
			return true
		}
		return false
	}
}

/// Counters for the iPad side of a session.
public struct SessionStatistics: Sendable, Equatable {
	/// `HELLO` packets sent in this session.
	public internal(set) var helloSent = 0

	/// Sessions accepted by the desktop.
	public internal(set) var sessions = 0

	/// `SAMPLE` packets sent.
	public internal(set) var samplesSent = 0

	/// Samples dropped because a newer position superseded them before sending.
	public internal(set) var samplesCoalesced = 0

	/// `HEARTBEAT` packets sent.
	public internal(set) var heartbeatsSent = 0

	/// `HEARTBEAT_ACK` packets received.
	public internal(set) var heartbeatsReceived = 0

	/// Times the desktop went silent for long enough to require a new `HELLO`.
	public internal(set) var timeouts = 0

	/// Most recent round-trip time, in microseconds.
	public internal(set) var lastRoundTripMicroseconds: UInt32?

	/// Most recent host clock minus device clock, in microseconds.
	public internal(set) var lastClockOffsetMicroseconds: Int64?

	/// Create empty statistics.
	public init() {}
}

/// Policy for one streaming session.
///
/// The desktop's defaults are a 750 ms watchdog and a 5000 ms session timeout.
/// The iPad sends samples at display rate while touching and a heartbeat once
/// per second, so an idle session stays alive without holding a contact.
public struct SessionConfiguration: Sendable, Equatable {
	/// Pairing token presented in every `HELLO`.
	public var token: PairingToken

	/// Capabilities advertised in `HELLO`.
	///
	/// The reference Pencil (USB-C) reports tilt and hover but not pressure, so
	/// the default omits `CAP_PRESSURE`.
	public var deviceCapabilities: PacketFlags

	/// Delay between repeated `HELLO` packets while connecting.
	public var helloInterval: Duration

	/// Delay between heartbeats while streaming.
	public var heartbeatInterval: Duration

	/// Silence from the desktop after which a new handshake starts.
	public var sessionTimeout: Duration

	/// Create a session policy.
	public init(
		token: PairingToken,
		deviceCapabilities: PacketFlags = [.capabilityTilt, .capabilityHover],
		helloInterval: Duration = .milliseconds(500),
		heartbeatInterval: Duration = .seconds(1),
		sessionTimeout: Duration = .seconds(5)
	) {
		self.token = token
		self.deviceCapabilities = deviceCapabilities
		self.helloInterval = helloInterval
		self.heartbeatInterval = heartbeatInterval
		self.sessionTimeout = sessionTimeout
	}
}
