import 'package:flutter/material.dart';
import 'package:fl_chart/fl_chart.dart';
import 'dart:math' as math;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../services/equity_data_provider.dart';

class EquityChart extends ConsumerStatefulWidget {
  final int maxPoints;

  const EquityChart({super.key, this.maxPoints = 1000});

  @override
  ConsumerState<EquityChart> createState() => _EquityChartState();
}

class _EquityChartState extends ConsumerState<EquityChart> {
  Map<String, double>? _cachedCurve;
  List<FlSpot>? _cachedSpots;
  int? _cachedMaxPoints;
  String? _cachedCurveHash;

  // P1B6R0F1#86: cached rendered widget + strict delta-check state so that
  // frequent WebSocket tick updates cannot trigger unbounded repaints.
  Widget? _cachedWidget;
  double? _lastRenderedFirstValue;
  double? _lastRenderedLastValue;
  int? _lastRenderedPointCount;
  static const double _repaintDeltaEpsilon = 1e-9;

  /// Strict delta check used to decide whether the chart must repaint.
  ///
  /// P1B6R0F1#86: the previous implementation rebuilt the entire [LineChart]
  /// subtree on every provider notification, even when the incoming tick
  /// changed the equity curve by a negligible amount (or not at all). This
  /// returns `false` (skip repaint) unless the curve's first value, last
  /// value, point count, or max-points configuration changed beyond a small
  /// relative epsilon.
  bool _shouldRepaint(Map<String, double> equityCurve, List<FlSpot> spots) {
    if (_cachedWidget == null ||
        _cachedMaxPoints != widget.maxPoints ||
        spots.isEmpty) {
      return true;
    }

    final pointCount = equityCurve.length;
    if (_lastRenderedPointCount != pointCount ||
        _lastRenderedFirstValue == null ||
        _lastRenderedLastValue == null) {
      return true;
    }

    final firstValue =
        equityCurve.values.isNotEmpty ? equityCurve.values.first : 0.0;
    final lastValue =
        equityCurve.values.isNotEmpty ? equityCurve.values.last : 0.0;

    final scale = math.max(1.0, math.max(firstValue.abs(), lastValue.abs()));
    final threshold = _repaintDeltaEpsilon * scale;
    final firstDelta = (firstValue - _lastRenderedFirstValue!).abs();
    final lastDelta = (lastValue - _lastRenderedLastValue!).abs();

    return firstDelta > threshold || lastDelta > threshold;
  }

  String _mapHash(Map<String, double> map) {
    final buffer = StringBuffer();
    for (final entry in map.entries) {
      buffer.write(entry.key);
      buffer.write(':');
      buffer.write(entry.value.toString());
      buffer.write(';');
    }
    return buffer.toString();
  }

  List<FlSpot> _downsample(List<num> data, int maxPoints) {
    final safeMax = maxPoints < 2 ? 2 : maxPoints;
    final n = data.length;
    if (n <= safeMax) {
      final spots = <FlSpot>[];
      for (var i = 0; i < n; i++) {
        final y = data[i].toDouble();
        if (!y.isFinite) continue;
        spots.add(FlSpot(i.toDouble(), y));
      }
      return spots;
    }
    if (safeMax <= 2) {
      final first = data.first.toDouble();
      final last = data.last.toDouble();
      final spots = <FlSpot>[];
      if (first.isFinite) spots.add(FlSpot(0, first));
      if (last.isFinite) spots.add(FlSpot((n - 1).toDouble(), last));
      return spots;
    }
    // Largest Triangle Three Buckets (LTTB) downsampling - O(n) single-pass.
    // Precompute bucket boundaries and averages in one linear scan,
    // then select max-area points in a second linear pass.
    final bucketSize = (n - 2) / (safeMax - 2);
    final numBuckets = safeMax - 2;
    
    // Bucket i covers data indices [bucketStart[i], bucketEnd[i])
    // for i = 1..numBuckets (middle buckets).
    // Bucket 0 = first point, bucket numBuckets+1 = last point.
    final bucketStart = List<int>.filled(numBuckets + 2, 0);
    final bucketEnd = List<int>.filled(numBuckets + 2, 0);
    final bucketAvgX = List<double>.filled(numBuckets + 2, 0.0);
    final bucketAvgY = List<double>.filled(numBuckets + 2, 0.0);
    
    bucketStart[0] = 0;
    bucketEnd[0] = 1;
    bucketAvgX[0] = 0.0;
    bucketAvgY[0] = data[0].toDouble();
    
    bucketStart[numBuckets + 1] = n - 1;
    bucketEnd[numBuckets + 1] = n;
    bucketAvgX[numBuckets + 1] = (n - 1).toDouble();
    bucketAvgY[numBuckets + 1] = data[n - 1].toDouble();
    
    // Single pass to compute bucket boundaries and averages.
    for (var i = 1; i <= numBuckets; i++) {
      final start = ((i - 1) * bucketSize).floor() + 1;
      final end = math.min((i * bucketSize).floor() + 1, n - 1);
      bucketStart[i] = start;
      bucketEnd[i] = end;
      if (end > start) {
        var sumX = 0.0;
        var sumY = 0.0;
        for (var j = start; j < end; j++) {
          sumX += j.toDouble();
          sumY += data[j].toDouble();
        }
        final count = end - start;
        bucketAvgX[i] = sumX / count;
        bucketAvgY[i] = sumY / count;
      }
    }
    
    final sampled = <FlSpot>[];
    if (bucketAvgY[0].isFinite) sampled.add(FlSpot(bucketAvgX[0], bucketAvgY[0]));
    
    var a = 0;
    for (var i = 1; i <= numBuckets; i++) {
      final avgX = bucketAvgX[i];
      final avgY = bucketAvgY[i];
      final rangeStart = bucketStart[i];
      final rangeEnd = bucketEnd[i];
      
      var maxArea = -1.0;
      var maxIdx = rangeStart;
      final ax = a.toDouble();
      final ay = data[a].toDouble();
      
      for (var j = rangeStart; j < rangeEnd; j++) {
        final bx = j.toDouble();
        final by = data[j].toDouble();
        final area = ((ax - avgX) * (by - ay) - (ay - avgY) * (bx - ax)).abs();
        if (area.isFinite && area > maxArea) {
          maxArea = area;
          maxIdx = j;
        }
      }
      
      final spotY = data[maxIdx].toDouble();
      if (spotY.isFinite) {
        sampled.add(FlSpot(maxIdx.toDouble(), spotY));
        a = maxIdx;
      }
    }
    
    if (bucketAvgY[numBuckets + 1].isFinite) {
      sampled.add(FlSpot(bucketAvgX[numBuckets + 1], bucketAvgY[numBuckets + 1]));
    }
    return sampled;
  }

  @override
  Widget build(BuildContext context) {
    // The parent rebuilds this widget with a fresh provider whenever the
    // curve changes; the hash and delta checks below skip redundant work.
    return Builder(
      builder: (context) {
        try {
          final equityCurve = ref.watch(equityDataProvider).getEquityCurve();
          if (equityCurve.isEmpty) {
            _cachedWidget = null;
            return const Center(child: Text('No equity data available'));
          }

          final currentHash = _mapHash(equityCurve);
          if (_cachedCurve == null ||
              _cachedCurveHash != currentHash ||
              _cachedMaxPoints != widget.maxPoints) {
            final equityValues = equityCurve.values.toList();
            _cachedSpots = _downsample(equityValues, widget.maxPoints);
            _cachedCurve = Map.from(equityCurve);
            _cachedCurveHash = currentHash;
            _cachedMaxPoints = widget.maxPoints;
          }

          final spots = _cachedSpots;
          if (spots == null || spots.isEmpty) {
            _cachedWidget = null;
            return const Center(child: Text('No valid equity data points'));
          }

          // P1B6R0F1#86: strict delta check — reuse the previously built
          // widget (and therefore skip the fl_chart repaint) when the curve
          // has not changed in a way that warrants a redraw.
          if (!_shouldRepaint(equityCurve, spots) && _cachedWidget != null) {
            return _cachedWidget!;
          }

          final equityValues = equityCurve.values.toList();
          final currentEquity = equityValues.isNotEmpty ? equityValues.last : 0.0;
          final firstEquity = equityValues.isNotEmpty ? equityValues.first : 0.0;
          final pct = firstEquity != 0 ? ((currentEquity - firstEquity) / firstEquity.abs()) * 100 : 0.0;
          final trend = pct > 0 ? 'up' : (pct < 0 ? 'down' : 'flat');

          final built = Semantics(
            label: 'Equity curve: \$${currentEquity.toStringAsFixed(2)}, change: ${pct.toStringAsFixed(2)}%, trend $trend',
            child: LineChart(
              LineChartData(
                gridData: FlGridData(show: true),
                titlesData: FlTitlesData(show: true),
                borderData: FlBorderData(show: true),
                lineBarsData: [
                  LineChartBarData(
                    spots: spots,
                    isCurved: true,
                    barWidth: 2,
                    color: Colors.blue,
                    dotData: FlDotData(show: false),
                  ),
                ],
                lineTouchData: LineTouchData(
                  enabled: true,
                  touchTooltipData: LineTouchTooltipData(
                    getTooltipItems: (touchedSpots) {
                      return touchedSpots.map((touchedSpot) {
                        final index = touchedSpot.x.toInt();
                        final equity = touchedSpot.y;
                        final keys = equityCurve.keys;
                        // The spot x is a downsampled source index; guard it so
                        // a stale/edge touch can never throw RangeError and
                        // blank the whole chart via the outer try/catch.
                        final timestamp = (index >= 0 && index < keys.length)
                            ? keys.elementAt(index).toString()
                            : '';
                        final prefix = timestamp.isEmpty ? '' : '$timestamp\n';
                        return LineTooltipItem(
                          '$prefix\$${equity.toStringAsFixed(2)}',
                          const TextStyle(color: Colors.white, fontWeight: FontWeight.bold),
                        );
                      }).toList();
                    },
                  ),
                  handleBuiltInTouches: true,
                ),
              ),
            ),
          );

          _cachedWidget = built;
          _lastRenderedFirstValue = firstEquity;
          _lastRenderedLastValue = currentEquity;
          _lastRenderedPointCount = equityCurve.length;
          return built;
        } catch (e) {
          return Center(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                const Icon(Icons.error_outline, color: Colors.red),
                const SizedBox(height: 8),
                Text('Failed to render equity chart: $e'),
              ],
            ),
          );
        }
      },
    );
  }
}

