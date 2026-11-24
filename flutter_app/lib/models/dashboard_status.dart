import 'package:flutter/foundation.dart';

@immutable
class DashboardStatus {
  // P1B1R0F7#46: version negotiation. The client advertises the protocol
  // versions it understands; an incoming payload must name one of them.
  static const int expectedVersion = 1;
  static const int defaultVersion = 1;
  static const List<int> supportedVersions = [1, 2];

  // JSON-schema validation per version: fields that MUST be present.
  // daily_pnl is deliberately absent: GET /api/status sends it as null until
  // the paper trader has a same-day baseline, and a missing or null value
  // means "unknown", never zero.
  static const Map<int, Set<String>> requiredFields = {
    1: {'equity', 'mode', 'trading_halted', 'active_positions'},
    2: {'equity', 'mode', 'trading_halted', 'active_positions'},
  };

  // P1B4R0F7#7: every field the constructor requires is declared here.
  final int version;
  final double equity;
  final String mode;
  final bool tradingHalted;
  final int activePositions;

  /// Today's PnL, or null when the server does not know it yet.
  final double? dailyPnl;
  final String? symbol;
  final String? haltReason;
  final bool isStale;
  final Map<String, dynamic> featureFlags;

  const DashboardStatus({
    this.version = expectedVersion,
    required this.equity,
    required this.mode,
    required this.tradingHalted,
    required this.activePositions,
    this.dailyPnl,
    this.symbol,
    this.haltReason,
    this.isStale = false,
    this.featureFlags = const {},
  });

  DashboardStatus copyWith({
    int? version,
    double? equity,
    String? mode,
    bool? tradingHalted,
    int? activePositions,
    double? dailyPnl,
    String? symbol,
    String? haltReason,
    bool? isStale,
    Map<String, dynamic>? featureFlags,
  }) {
    return DashboardStatus(
      version: version ?? this.version,
      equity: equity ?? this.equity,
      mode: mode ?? this.mode,
      tradingHalted: tradingHalted ?? this.tradingHalted,
      activePositions: activePositions ?? this.activePositions,
      dailyPnl: dailyPnl ?? this.dailyPnl,
      symbol: symbol ?? this.symbol,
      haltReason: haltReason ?? this.haltReason,
      isStale: isStale ?? this.isStale,
      featureFlags: featureFlags ?? this.featureFlags,
    );
  }

  /// Re-validate invariants a malformed server response could violate even
  /// when [fromJson] was bypassed or a payload was hand-built.
  void validate() {
    if (!supportedVersions.contains(version)) {
      throw FormatException('Unsupported protocol version: $version');
    }
    if (!equity.isFinite || equity < 0) {
      throw ArgumentError('Invalid equity: must be finite and non-negative');
    }
    if (activePositions < 0) {
      throw FormatException('active_positions cannot be negative');
    }
    if (mode.isEmpty) {
      throw FormatException('mode must be a non-empty string');
    }
    if (dailyPnl != null && !dailyPnl!.isFinite) {
      throw ArgumentError('Invalid daily_pnl: must be finite');
    }
  }

  factory DashboardStatus.fromJson(Map<String, dynamic> json) {
    // Version negotiation: an absent version means the default; an
    // unsupported version fails closed instead of being coerced, so a server
    // can add fields and bump the version without silently breaking clients.
    // Version may arrive as an int, or as a string when a proxy/serializer
    // stringifies it; coerce both instead of hard-casting to int? (which would
    // throw a TypeError on "1"). Fall back to defaultVersion when absent or
    // unparseable.
    final Object? rawVersion = json['version'];
    final int incomingVersion = rawVersion is num
        ? rawVersion.toInt()
        : rawVersion is String
            ? int.tryParse(rawVersion) ?? defaultVersion
            : defaultVersion;
    if (!supportedVersions.contains(incomingVersion)) {
      throw FormatException(
        'Unsupported protocol version: $incomingVersion. This client '
        'supports protocol versions $supportedVersions (expected '
        '$expectedVersion). Upgrade the client or migrate the server '
        'payload to a supported version.',
      );
    }

    // JSON schema validation per version: all required fields must exist.
    final Set<String> required = requiredFields[incomingVersion] ?? const <String>{};
    for (final String field in required) {
      if (!json.containsKey(field)) {
        throw FormatException('Missing required field for version $incomingVersion: $field');
      }
    }

    final Object? rawEquity = json['equity'];
    if (rawEquity is! num) {
      throw FormatException('equity must be numeric; got $rawEquity');
    }
    final double equityValue = rawEquity.toDouble();
    if (!equityValue.isFinite || equityValue < 0) {
      throw ArgumentError('Invalid equity: must be finite and non-negative');
    }

    // P1B6R0F7#18: reject a negative position count. A buggy backend
    // returning -1 must fail loudly here rather than being silently accepted
    // and rendered as a nonsensical negative position count.
    final Object? rawPositions = json['active_positions'];
    if (rawPositions is! num) {
      throw FormatException('active_positions must be numeric; got $rawPositions');
    }
    final int activePositionsValue = rawPositions.toInt();
    if (activePositionsValue < 0) {
      throw FormatException('active_positions cannot be negative');
    }

    final Object? rawPnl = json['daily_pnl'];
    if (rawPnl != null && rawPnl is! num) {
      throw FormatException('daily_pnl must be numeric or null');
    }
    final double? dailyPnlValue = (rawPnl as num?)?.toDouble();
    if (dailyPnlValue != null && !dailyPnlValue.isFinite) {
      throw ArgumentError('Invalid daily_pnl: must be finite');
    }

    // Version-specific handling: optional fields added in v2 are read here.
    // requiredFields keeps only the universally-required keys, so a v2 server
    // may add optional fields without breaking v1 clients.
    final Map<String, dynamic> featureFlagsValue =
        (json['feature_flags'] as Map<dynamic, dynamic>?)?.map(
              (k, v) => MapEntry(k.toString(), v),
            ) ??
            const {};

    // The mode is the server's word ("paper" today); it is shown as sent, and
    // a missing or non-string mode fails closed rather than being invented.
    final Object? rawMode = json['mode'];
    if (rawMode is! String || rawMode.isEmpty) {
      throw FormatException('mode must be a non-empty string; got $rawMode');
    }

    final Object? rawHalted = json['trading_halted'];
    if (rawHalted is! bool) {
      throw FormatException('trading_halted must be a bool; got $rawHalted');
    }

    return DashboardStatus(
      version: incomingVersion,
      equity: equityValue,
      mode: rawMode,
      tradingHalted: rawHalted,
      activePositions: activePositionsValue,
      dailyPnl: dailyPnlValue,
      symbol: json['symbol'] as String?,
      haltReason: json['haltReason'] as String?,
      isStale: json['isStale'] as bool? ?? false,
      featureFlags: featureFlagsValue,
    );
  }

  Map<String, dynamic> toJson() => {
        'version': version,
        'equity': equity,
        'mode': mode,
        'trading_halted': tradingHalted,
        'active_positions': activePositions,
        'daily_pnl': dailyPnl,
        'symbol': symbol,
        'haltReason': haltReason,
        'isStale': isStale,
        'feature_flags': featureFlags,
      };

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is DashboardStatus &&
          runtimeType == other.runtimeType &&
          version == other.version &&
          equity == other.equity &&
          mode == other.mode &&
          tradingHalted == other.tradingHalted &&
          activePositions == other.activePositions &&
          dailyPnl == other.dailyPnl &&
          symbol == other.symbol &&
          haltReason == other.haltReason &&
          isStale == other.isStale &&
          _mapEquals(featureFlags, other.featureFlags);

  @override
  int get hashCode => Object.hash(
        version,
        equity,
        mode,
        tradingHalted,
        activePositions,
        dailyPnl,
        symbol,
        haltReason,
        isStale,
        Object.hashAllUnordered(
          featureFlags.entries.map((e) => Object.hash(e.key, e.value)),
        ),
      );

  static bool _mapEquals(Map<String, dynamic> a, Map<String, dynamic> b) {
    if (a.length != b.length) return false;
    for (final String key in a.keys) {
      if (!b.containsKey(key) || a[key] != b[key]) return false;
    }
    return true;
  }
}
