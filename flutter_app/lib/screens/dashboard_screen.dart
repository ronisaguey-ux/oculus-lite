import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart';
import 'package:provider/provider.dart';

import '../models/dashboard_status.dart';
import '../models/risk_summary.dart';
import '../services/equity_data_provider.dart';
import '../services/state_provider.dart';
import '../widgets/drawdown_gauge.dart';
import '../widgets/equity_chart.dart';
import '../widgets/kill_switch_card.dart';
import '../widgets/order_ticket_card.dart';
import '../widgets/trade_analytics_summary.dart';
import 'risk_tab.dart';

/// The signed-in dashboard: Overview, Risk, Strategies and Trades, each fed
/// by one route of oculus/api_server.py through [OculusState].
class DashboardScreen extends StatelessWidget {
  const DashboardScreen({super.key, this.onLogout});

  final VoidCallback? onLogout;

  @override
  Widget build(BuildContext context) {
    final isLoading = context.select<OculusState, bool>((s) => s.isLoading);
    final hasStatus = context.select<OculusState, bool>((s) => s.status != null);
    final error = context.select<OculusState, String?>((s) => s.error);

    final actions = <Widget>[
      if (onLogout != null)
        IconButton(
          tooltip: 'Log out',
          icon: const Icon(Icons.logout),
          onPressed: onLogout,
        ),
    ];

    if (isLoading && !hasStatus) {
      return const Scaffold(body: Center(child: CircularProgressIndicator()));
    }
    if (!hasStatus) {
      return Scaffold(
        appBar: AppBar(title: const Text('Oculus Dashboard'), actions: actions),
        body: Center(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Text('Status unavailable: ${error ?? 'unknown error'}'),
              const SizedBox(height: 12),
              ElevatedButton(
                onPressed: () => context.read<OculusState>().refresh(),
                child: const Text('Retry'),
              ),
            ],
          ),
        ),
      );
    }

    return DefaultTabController(
      length: 4,
      child: Scaffold(
        appBar: AppBar(
          title: const Text('Oculus Dashboard'),
          actions: actions,
          bottom: const TabBar(
            isScrollable: true,
            tabs: [
              Tab(text: 'Overview', icon: Icon(Icons.dashboard_outlined)),
              Tab(text: 'Risk', icon: Icon(Icons.warning_amber_outlined)),
              Tab(text: 'Strategies', icon: Icon(Icons.psychology_outlined)),
              Tab(text: 'Trades', icon: Icon(Icons.receipt_long_outlined)),
            ],
          ),
        ),
        body: const TabBarView(
          children: [OverviewTab(), RiskTab(), StrategiesTab(), TradesTab()],
        ),
      ),
    );
  }
}

String _money(double v) => '${v < 0 ? '-' : ''}\$${v.abs().toStringAsFixed(2)}';

class OverviewTab extends StatelessWidget {
  const OverviewTab({super.key});

  @override
  Widget build(BuildContext context) {
    final status = context.select<OculusState, DashboardStatus?>((s) => s.status);
    final liveEquity = context.select<OculusState, double?>((s) => s.liveEquity);
    final connected = context.select<OculusState, bool>((s) => s.isConnected);
    final risk = context.select<OculusState, RiskSummary?>((s) => s.riskSummary);
    final trades = context.select<OculusState, List<Map<String, dynamic>>>((s) => s.trades);
    final tradesTotal = context.select<OculusState, int>((s) => s.tradesTotal);
    if (status == null) return const SizedBox.shrink();

    final equity = liveEquity ?? status.equity;
    final pnl = status.dailyPnl;
    final items = <(String, String)>[
      ('Mode', status.mode),
      if (status.symbol != null) ('Symbol', status.symbol!),
      ('Paper trading halted', status.tradingHalted ? 'Yes${status.haltReason == null ? '' : ' (${status.haltReason})'}' : 'No'),
      ('Active positions', '${status.activePositions}'),
      ('Daily PnL', pnl == null ? 'Unknown' : _money(pnl)),
      ('Live stream', connected ? 'Connected' : 'Disconnected'),
    ];

    return RefreshIndicator(
      onRefresh: () => context.read<OculusState>().refresh(),
      child: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Semantics(
            sortKey: const OrdinalSortKey(1),
            label: 'Current equity ${_money(equity)}',
            excludeSemantics: true,
            child: Text('Equity: ${_money(equity)}',
                style: Theme.of(context).textTheme.headlineMedium),
          ),
          if (status.isStale)
            const Text('Status is stale: the paper trader has not updated it recently.'),
          const SizedBox(height: 8),
          const KillSwitchCard(),
          const SizedBox(height: 8),
          const OrderTicketCard(),
          for (final (i, (label, value)) in items.indexed)
            Semantics(
              sortKey: OrdinalSortKey(i + 2.0),
              label: '$label $value',
              excludeSemantics: true,
              child: Padding(
                padding: const EdgeInsets.symmetric(vertical: 2),
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [Text(label), Text(value)],
                ),
              ),
            ),
          if (risk != null) ...[
            const SizedBox(height: 16),
            DrawdownGauge(
              label: 'Rolling ${risk.windowHours}h drawdown',
              drawdown: risk.rollingDd,
              limit: risk.rollingDailyDdLimit,
            ),
            const SizedBox(height: 12),
            DrawdownGauge(
              label: 'Static drawdown',
              drawdown: risk.staticDd,
              limit: risk.staticDdKillLimit,
            ),
          ],
          const SizedBox(height: 16),
          TradeAnalyticsSummary(trades: trades, total: tradesTotal),
          const SizedBox(height: 16),
          SizedBox(
            height: 300,
            child: RepaintBoundary(
              child: Selector<OculusState, Map<String, dynamic>>(
                selector: (_, state) => state.equityCurve,
                builder: (context, curve, _) =>
                    EquityChart(provider: JsonEquityCurve(curve)),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// Champion-ensemble members from /api/tournament_leaderboard.
class StrategiesTab extends StatelessWidget {
  const StrategiesTab({super.key});

  @override
  Widget build(BuildContext context) {
    final leaders = context.select<OculusState, List<Map<String, dynamic>>>((s) => s.leaders);
    final error = context.select<OculusState, String?>((s) => s.leaderboardError);
    if (leaders.isEmpty) {
      return Center(child: Text(error == null ? 'No strategies available' : 'Strategies unavailable: $error'));
    }
    return RefreshIndicator(
      onRefresh: () => context.read<OculusState>().refresh(),
      child: ListView.builder(
        padding: const EdgeInsets.all(16),
        itemCount: leaders.length,
        itemBuilder: (context, index) {
          final m = leaders[index];
          final rank = m['rank'] ?? index + 1;
          final ensemble = (m['ensemble'] ?? 'unknown').toString();
          final symbol = (m['symbol'] ?? '').toString();
          final bar = m['barMinutes'];
          final fitness = m['fitness'];
          final holdout = m['holdoutPassed'];
          final subtitle = [
            if (symbol.isNotEmpty) symbol,
            if (bar is num) '${bar.toInt()}m bars',
            fitness is num ? 'fitness ${fitness.toStringAsFixed(4)}' : 'no fitness',
            holdout == true ? 'holdout passed' : holdout == false ? 'holdout failed' : 'holdout n/a',
          ].join(' · ');
          return Semantics(
            sortKey: OrdinalSortKey(index.toDouble()),
            label: 'Rank $rank, $ensemble, $subtitle',
            excludeSemantics: true,
            child: Card(
              child: ListTile(
                leading: Text('#$rank'),
                title: Text(ensemble),
                subtitle: Text(subtitle),
              ),
            ),
          );
        },
      ),
    );
  }
}

/// Closed trades from /api/trades, newest first.
class TradesTab extends StatelessWidget {
  const TradesTab({super.key});

  @override
  Widget build(BuildContext context) {
    final trades = context.select<OculusState, List<Map<String, dynamic>>>((s) => s.trades);
    final loading = context.select<OculusState, bool>((s) => s.tradesLoading);
    final error = context.select<OculusState, String?>((s) => s.tradesError);
    if (loading && trades.isEmpty) {
      return const Center(child: CircularProgressIndicator());
    }
    if (error != null) {
      return Center(child: Text('Error loading trades: $error'));
    }
    if (trades.isEmpty) {
      return const Center(child: Text('No trades available'));
    }
    final newestFirst = trades.reversed.toList(growable: false);
    return RefreshIndicator(
      onRefresh: () => context.read<OculusState>().refresh(),
      child: ListView.builder(
        padding: const EdgeInsets.all(16),
        itemCount: newestFirst.length,
        itemBuilder: (context, index) {
          final t = newestFirst[index];
          final symbol = (t['symbol'] ?? 'Unknown').toString();
          final direction = (t['direction'] ?? '').toString();
          final pnl = t['pnl'];
          final pnlText = pnl is num ? _money(pnl.toDouble()) : 'n/a';
          final exit = (t['exitTime'] ?? '').toString();
          return Semantics(
            sortKey: OrdinalSortKey(index.toDouble()),
            label: 'Trade $symbol $direction, profit and loss $pnlText',
            excludeSemantics: true,
            child: Card(
              child: ListTile(
                title: Text('$symbol $direction'.trim()),
                subtitle: Text(exit),
                trailing: Text(pnlText),
              ),
            ),
          );
        },
      ),
    );
  }
}
