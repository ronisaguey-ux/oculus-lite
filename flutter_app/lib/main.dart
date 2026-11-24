import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:provider/provider.dart';

import 'screens/dashboard_screen.dart';
import 'services/api_client.dart';
import 'services/state_provider.dart';

/// The lite dashboard talks only to the lite engine's own paper API.
/// Override at build time with `--dart-define=OCULUS_LITE_API=<url>`.
const String kLiteApiBaseUrl = String.fromEnvironment(
  'OCULUS_LITE_API',
  defaultValue: 'http://127.0.0.1:8080',
);

void main() {
  runApp(const ProviderScope(child: OculusLiteApp()));
}

/// Composition root. No login, no credentials, no private endpoints: the paper
/// API is read-only and local by default.
class OculusLiteApp extends StatelessWidget {
  const OculusLiteApp({super.key, this.baseUrl = kLiteApiBaseUrl});

  final String baseUrl;

  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider<OculusState>(
      create: (_) {
        final state = OculusState(apiClient: ApiClient(baseUrl: baseUrl));
        state.refresh();
        return state;
      },
      child: MaterialApp(
        title: 'oculus-lite',
        debugShowCheckedModeBanner: false,
        theme: ThemeData(brightness: Brightness.light, colorSchemeSeed: Colors.blue),
        darkTheme: ThemeData(brightness: Brightness.dark, colorSchemeSeed: Colors.blue),
        home: const DashboardScreen(),
      ),
    );
  }
}
