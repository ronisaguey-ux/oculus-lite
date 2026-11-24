import 'dart:convert';

import 'package:http/http.dart' as http;

import '../exceptions/api_exception.dart';

/// HTTP client for the oculus-lite paper API served by `oculus-paper-api`.
///
/// Trimmed from the original companion client: no login, no tokens, no
/// websocket, no order placement. Every route is a read-only GET.
class ApiClient {
  ApiClient({
    required String baseUrl,
    http.Client? httpClient,
    this.timeout = const Duration(seconds: 10),
  })  : _base = Uri.parse(baseUrl),
        _http = httpClient ?? http.Client(),
        _ownsHttp = httpClient == null;

  final Uri _base;
  final http.Client _http;
  final bool _ownsHttp;
  final Duration timeout;

  Uri get baseUri => _base;

  Uri _uri(String path) => _base.replace(path: '${_base.path}$path');

  Future<Map<String, dynamic>> _getMap(String path) async {
    final http.Response response;
    try {
      response = await _http.get(_uri(path)).timeout(timeout);
    } on ApiException {
      rethrow;
    } catch (e) {
      throw NetworkError('GET $path failed: $e', endpoint: path, method: 'GET');
    }
    if (response.statusCode < 200 || response.statusCode >= 300) {
      throw ApiException(
        response.statusCode,
        response.body.isEmpty ? 'HTTP ${response.statusCode}' : response.body,
        endpoint: path,
        method: 'GET',
        rawBody: response.body,
      );
    }
    final Object? decoded;
    try {
      decoded = jsonDecode(response.body);
    } on FormatException {
      throw ApiException(502, 'Malformed JSON from $path', endpoint: path, method: 'GET', rawBody: response.body);
    }
    if (decoded is! Map<String, dynamic>) {
      throw ApiException(502, 'Expected a JSON object from $path', endpoint: path, method: 'GET', rawBody: response.body);
    }
    return decoded;
  }

  /// GET /api/status -> DashboardStatus-compatible object.
  Future<Map<String, dynamic>> getStatus() => _getMap('/api/status');

  /// GET /api/equity_curve -> flat `{isoTimestamp: equity}` map.
  Future<Map<String, dynamic>> getEquityCurve() => _getMap('/api/equity_curve');

  /// GET /api/trades -> `{trades: [...]}`.
  Future<Map<String, dynamic>> getTrades({int limit = 100}) => _getMap('/api/trades');

  /// GET /api/positions -> `{positions: [...]}`.
  Future<Map<String, dynamic>> getPositions() => _getMap('/api/positions');

  /// GET /api/fills -> `{fills: [...]}`.
  Future<Map<String, dynamic>> getFills({int limit = 100}) => _getMap('/api/fills');

  /// GET /api/parameters -> `{parameters: [{category,name,value}]}`.
  Future<Map<String, dynamic>> getParameters() => _getMap('/api/parameters');

  void close() {
    if (_ownsHttp) _http.close();
  }
}
