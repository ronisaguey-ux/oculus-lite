/// State provider for the Oculus Flutter app.
///
/// [StateProvider] owns reactive state slices fed by the server contract
/// (oculus/api_server.py). High-frequency updates are batched to reduce
/// notification storms and improve UI performance.
library;

import 'dart:async';

import 'package:flutter/foundation.dart';

import '../models/position.dart';
import '../models/risk_alert.dart';
import '../models/user.dart';
import 'api_client.dart';

/// Immutable application state.
class AppState {
  final User? user;
  final List<Position> positions;
  final List<RiskAlert> alerts;
  final bool isKillSwitchEngaged;

  const AppState({
    this.user,
    this.positions = const [],
    this.alerts = const [],
    this.isKillSwitchEngaged = false,
  });

  AppState copyWith({
    User? user,
    List<Position>? positions,
    List<RiskAlert>? alerts,
    bool? isKillSwitchEngaged,
  }) {
    return AppState(
      user: user ?? this.user,
      positions: positions ?? this.positions,
      alerts: alerts ?? this.alerts,
      isKillSwitchEngaged: isKillSwitchEngaged ?? this.isKillSwitchEngaged,
    );
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is AppState &&
          runtimeType == other.runtimeType &&
          user == other.user &&
          _listEquals(positions, other.positions) &&
          _listEquals(alerts, other.alerts) &&
          isKillSwitchEngaged == other.isKillSwitchEngaged;

  @override
  int get hashCode =>
      user.hashCode ^
      positions.hashCode ^
      alerts.hashCode ^
      isKillSwitchEngaged.hashCode;
}

bool _listEquals<T>(List<T> a, List<T> b) {
  if (a.length != b.length) return false;
  for (int i = 0; i < a.length; i++) {
    if (a[i] != b[i]) return false;
  }
  return true;
}

/// Reactive state provider that batches high-frequency updates.
///
/// Uses a 100ms coalescing window to reduce rebuilds during rapid data changes,
/// improving UI performance and reducing notification storms.
class StateProvider extends ChangeNotifier {
  StateProvider({required this.apiClient});

  final ApiClient apiClient;

  AppState _state = const AppState();
  AppState get state => _state;

  bool _disposed = false;

  // Batching for high-frequency updates
  Timer? _notifyBatchTimer;
  static const Duration _notifyBatchWindow = Duration(milliseconds: 100);

  void _notify({bool immediate = false}) {
    if (_disposed) return;
    if (immediate) {
      _notifyBatchTimer?.cancel();
      _notifyBatchTimer = null;
      notifyListeners();
      return;
    }
    _notifyBatchTimer ??= Timer(_notifyBatchWindow, () {
      _notifyBatchTimer = null;
      if (!_disposed) notifyListeners();
    });
  }

  /// Update user state.
  void setUser(User? user) {
    _state = _state.copyWith(user: user);
    _notify();
  }

  /// Update positions state.
  void setPositions(List<Position> positions) {
    _state = _state.copyWith(positions: positions);
    _notify();
  }

  /// Update alerts state.
  void setAlerts(List<RiskAlert> alerts) {
    _state = _state.copyWith(alerts: alerts);
    _notify();
  }

  /// Update kill switch state.
  void setKillSwitchEngaged(bool engaged) {
    _state = _state.copyWith(isKillSwitchEngaged: engaged);
    _notify(immediate: true);
  }

  /// Refresh all state from the API.
  Future<void> refresh() async {
    await Future.wait<void>([
      _loadUser(),
      _loadPositions(),
      _loadAlerts(),
      _loadKillSwitch(),
    ]);
    _notify(immediate: true);
  }

  Future<void> _loadUser() async {
    try {
      final user = await apiClient.getCurrentUser();
      _state = _state.copyWith(user: user);
    } catch (e) {
      debugPrint('StateProvider: failed to load user: $e');
    }
  }

  Future<void> _loadPositions() async {
    try {
      final body = await apiClient.getTrades(limit: 100);
      final raw = body['trades'];
      if (raw is List) {
        final positions = raw
            .whereType<Map<String, dynamic>>()
            .map((p) => Position.fromJson(p))
            .toList();
        _state = _state.copyWith(positions: positions);
      }
    } catch (e) {
      debugPrint('StateProvider: failed to load positions: $e');
    }
  }

  Future<void> _loadAlerts() async {
    try {
      final alerts = await apiClient.getRiskAlerts();
      _state = _state.copyWith(alerts: alerts);
    } catch (e) {
      debugPrint('StateProvider: failed to load alerts: $e');
    }
  }

  Future<void> _loadKillSwitch() async {
    try {
      final body = await apiClient.getHaltStatus();
      final halted = body['halted'] == true;
      _state = _state.copyWith(isKillSwitchEngaged: halted);
    } catch (e) {
      debugPrint('StateProvider: failed to load kill switch: $e');
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _notifyBatchTimer?.cancel();
    super.dispose();
  }
}
