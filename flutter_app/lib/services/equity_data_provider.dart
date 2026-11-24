/// Signature for a function returning the current equity curve as a map of
/// timestamp keys to values.
///
/// The returned map must not be mutated by callers; implementations should
/// return a defensive copy to protect internal state.
typedef EquityCurveSource = Map<String, double> Function();

/// Abstract contract for equity-curve data providers consumed by the Flutter
/// dashboard (e.g. `EquityChart`). Implementations stream or fetch the current
/// equity curve and expose it as a timestamp->value map. Callers must not
/// mutate the returned map; implementations should return a defensive copy.
abstract class EquityDataProvider {
  Map<String, double> getEquityCurve();
}

/// Adapts the decoded GET /api/equity_curve body (`{isoTimestamp: number}`)
/// to [EquityDataProvider].
///
/// JSON numbers decode as `int` or `double` (a whole-dollar equity arrives as
/// an int), so every value goes through `num.toDouble()`; a hard `as double`
/// cast would throw on the first int. Non-numeric and non-finite values are
/// dropped rather than charted.
class JsonEquityCurve implements EquityDataProvider {
  JsonEquityCurve(Map<String, dynamic> json) : _curve = _parse(json);

  final Map<String, double> _curve;

  static Map<String, double> _parse(Map<String, dynamic> json) {
    final out = <String, double>{};
    for (final entry in json.entries) {
      final value = entry.value;
      if (value is num) {
        final d = value.toDouble();
        if (d.isFinite) out[entry.key] = d;
      }
    }
    return out;
  }

  @override
  Map<String, double> getEquityCurve() => Map<String, double>.of(_curve);
}

/// Cached implementation with TTL (time-to-live), validation, stale-read
/// protection, defensive copies and linear interpolation for missing intervals.
class CachedEquityDataProvider implements EquityDataProvider {
  final EquityCurveSource _delegate;
  final Duration _ttl; // TTL for internal use

  _CacheEntry? _entry;

  /// P1B2R0F3#211: multi-key cache used by [add] so eviction can be exercised
  /// with more than one key. [getEquityCurve] keeps its single fixed-key path
  /// via [_entry] for backward compatibility.
  final Map<String, _CacheEntry> _cache = <String, _CacheEntry>{};

  /// Maximum number of entries retained in [_cache] before the oldest is
  /// evicted.
  final int maxCacheSize;

  /// Maximum age of a data point relative to now before it is treated as
  /// stale and dropped during validation/normalization.
  final Duration maxDataAge;

  /// Tolerance for clock skew when rejecting future-dated timestamps.
  final Duration futureSkewTolerance;

  /// Source of "now" for TTL, staleness and future-skew checks; injectable so
  /// tests with fixed timestamps do not rot as the wall clock moves on.
  final DateTime Function() _clock;

  CachedEquityDataProvider(this._delegate,
      {Duration ttl = const Duration(seconds: 60),
      this.maxDataAge = const Duration(hours: 24),
      this.futureSkewTolerance = const Duration(minutes: 5),
      this.maxCacheSize = 2,
      DateTime Function()? clock})
      : _ttl = ttl,
        _clock = clock ?? DateTime.now;

  @override
  Map<String, double> getEquityCurve() {
    final now = _clock();

    // Check if we have a valid cache entry - return defensive copy
    if (_entry != null && now.difference(_entry!.timestamp) <= _ttl) {
      return Map<String, double>.from(_entry!.data);
    }

    // Compute new value with validation
    final rawData = _delegate();

    // Validation and interpolation are linear in the curve length and run
    // synchronously; the curve is a few thousand points at most.
    final sanitizedData = <String, double>{};
    DateTime? lastTimestamp;
    for (final entry in rawData.entries) {
      final key = entry.key;
      final value = entry.value;
      if (value.isNaN || value.isInfinite) continue;
      if (!_isValidTimestampKey(key)) continue;
      final timestamp = _parseTimestamp(key);
      if (timestamp == null) continue;
      if (timestamp.isAfter(now.add(futureSkewTolerance))) continue;
      if (now.difference(timestamp) > maxDataAge) continue;
      if (lastTimestamp != null && timestamp.isBefore(lastTimestamp)) continue;
      lastTimestamp = timestamp;
      sanitizedData[key] = value;
    }

    if (sanitizedData.isEmpty) {
      if (_entry != null) {
        return Map<String, double>.from(_entry!.data);
      }
      return <String, double>{};
    }

    final interpolated = _interpolateMissingPoints(sanitizedData);
    _entry = _CacheEntry(Map<String, double>.from(interpolated), now);
    return Map<String, double>.from(interpolated);
  }

  bool _isValidTimestampKey(String key) {
    if (key.isEmpty) return false;
    try {
      DateTime.parse(key);
      return true;
    } catch (_) {}
    final epoch = int.tryParse(key);
    if (epoch != null && epoch > 0) return true;
    return false;
  }

  DateTime? _parseTimestamp(String key) {
    try {
      return DateTime.parse(key);
    } catch (_) {}
    final epoch = int.tryParse(key);
    if (epoch != null) {
      if (epoch > 10000000000) {
        return DateTime.fromMillisecondsSinceEpoch(epoch);
      } else {
        return DateTime.fromMillisecondsSinceEpoch(epoch * 1000);
      }
    }
    return null;
  }

  String _formatTimestamp(DateTime dt) => dt.toIso8601String();

  Map<String, double> _interpolateMissingPoints(Map<String, double> data) {
    if (data.length < 2) return data;
    final entries = <MapEntry<DateTime, double>>[];
    for (final e in data.entries) {
      final dt = _parseTimestamp(e.key);
      if (dt != null) entries.add(MapEntry(dt, e.value));
    }
    if (entries.length < 2) return data;
    entries.sort((a, b) => a.key.compareTo(b.key));
    final deltas = <Duration>[];
    for (var i = 1; i < entries.length; i++) {
      deltas.add(entries[i].key.difference(entries[i-1].key));
    }
    deltas.sort((a,b) => a.inMilliseconds.compareTo(b.inMilliseconds));
    final medianDelta = deltas[deltas.length ~/ 2];
    if (medianDelta.inMilliseconds <= 0) return data;
    final result = <String, double>{};
    for (var i = 0; i < entries.length; i++) {
      final curr = entries[i];
      result[_formatTimestamp(curr.key)] = curr.value;
      if (i + 1 < entries.length) {
        final next = entries[i+1];
        final gap = next.key.difference(curr.key);
        if (gap.inMilliseconds > medianDelta.inMilliseconds * 1.5) {
          final steps = (gap.inMilliseconds / medianDelta.inMilliseconds).round();
          if (steps > 1) {
            for (var s = 1; s < steps; s++) {
              final fraction = s / steps;
              final interpTime = curr.key.add(Duration(milliseconds: medianDelta.inMilliseconds * s));
              if (interpTime.isAfter(next.key) || interpTime.isAtSameMomentAs(next.key)) break;
              final interpValue = curr.value + (next.value - curr.value) * fraction;
              result[_formatTimestamp(interpTime)] = interpValue;
            }
          }
        }
      }
    }
    return result;
  }

  /// P1B2R0F3#211: testable insertion point that keys a cache entry by [key]
  /// and evicts the oldest entry once [maxCacheSize] is exceeded. This gives
  /// the eviction test a real N+1 scenario instead of a placeholder.
  void add(String key, Map<String, double> data, {DateTime? timestamp}) {
    final now = timestamp ?? _clock();
    _cache.remove(key);
    _cache[key] = _CacheEntry(Map<String, double>.from(data), now);
    // Evict by entry timestamp, not insertion order: an entry added late
    // with an old timestamp is the oldest data and goes first.
    while (_cache.length > maxCacheSize) {
      final oldest = _cache.entries
          .reduce((a, b) => b.value.timestamp.isBefore(a.value.timestamp) ? b : a)
          .key;
      _cache.remove(oldest);
    }
  }

  /// Number of entries currently retained in the multi-key cache.
  int get cacheSize => _cache.length;

  /// Whether an entry for [key] is present in the multi-key cache.
  bool containsKey(String key) => _cache.containsKey(key);

  void clearCache() {
    _entry = null;
    _cache.clear();
  }

  /// Invalidates the cache immediately, forcing the next call to
  /// [getEquityCurve] to re-fetch from the underlying source.
  void invalidate() {
    _entry = null;
  }

  /// Releases everything this provider holds: the single-entry cache, the
  /// multi-key cache, and (when the delegate supports it) the delegate itself.
  ///
  /// The delegate is typed as a plain [EquityCurveSource] function, so a
  /// resource-backed implementation that also exposes `dispose()` is forwarded
  /// to dynamically; delegates without one are simply left alone. Without this
  /// the delegate's timers/streams/file handles would outlive the provider and
  /// leak (memory and file descriptors).
  void dispose() {
    _entry = null;
    _cache.clear();
    final dynamic delegate = _delegate;
    try {
      delegate.dispose();
    } catch (_) {
      // Delegate does not expose dispose(); try close() below.
    }
    try {
      delegate.close();
    } catch (_) {
      // Delegate exposes neither dispose() nor close(); nothing to release.
    }
  }
}

class _CacheEntry {
  final Map<String, double> data;
  final DateTime timestamp;

  _CacheEntry(this.data, this.timestamp);
}
