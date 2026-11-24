import 'dart:convert';

enum ApiErrorCode {
  badRequest,
  unauthorized,
  forbidden,
  notFound,
  rateLimited,
  serverError,
  networkError,
  unknown;

  factory ApiErrorCode.fromStatusCode(int? statusCode) {
    switch (statusCode) {
      case 400:
        return ApiErrorCode.badRequest;
      case 401:
        return ApiErrorCode.unauthorized;
      case 403:
        return ApiErrorCode.forbidden;
      case 404:
        return ApiErrorCode.notFound;
      case 429:
        return ApiErrorCode.rateLimited;
      case 500:
      case 501:
      case 502:
      case 503:
      case 504:
        return ApiErrorCode.serverError;
      default:
        return ApiErrorCode.unknown;
    }
  }

  bool get retryable => switch (this) {
        ApiErrorCode.rateLimited ||
        ApiErrorCode.serverError ||
        ApiErrorCode.networkError =>
          true,
        _ => false,
      };
}

/// Exception carrying full request context (endpoint, method, request ID,
/// timestamp, retry count) so production incidents are debuggable.
///
/// P1B5R0F13#44: adds structured JSON logging via [toLogJson] with a
/// correlation ID ([requestId]) and a sanitized message, so log aggregation
/// can correlate an error back to the exact request that produced it.
///
/// P1B7R0F9#36: unified the structurally identical `ApiException` and
/// `TradingRejectedException` into this single parameterized exception.
/// The previous name is retained as a deprecated alias for compatibility.
sealed class ApiException implements Exception {
  final int? statusCode;
  final String _message;
  final String? endpoint;
  final String? method;
  final String requestId;
  final DateTime timestamp;
  final int retryCount;
  final String? rawBody;
  final String? errorCode;
  final Map<String, dynamic>? details;

  /// Seconds the server asked the client to wait before retrying, parsed from
  /// a `Retry-After` header (429/503 responses). `null` when not supplied.
  final int? retryAfter;

  ApiException._(
    this.statusCode,
    String? message, {
    this.endpoint,
    this.method,
    String? requestId,
    DateTime? timestamp,
    this.retryCount = 0,
    this.rawBody,
    this.errorCode,
    this.details,
    this.retryAfter,
  })  : _message = message ?? '',
        requestId = requestId ?? _generateRequestId(),
        timestamp = timestamp ?? DateTime.now().toUtc() {
    final code = statusCode;
    if (code != null && (code < 100 || code >= 600)) {
      throw ArgumentError.value(
          code, 'statusCode', 'must be a valid HTTP status code (100-599)');
    }
  }

  /// CE105-1701: public factory preserving the historical unnamed constructor
  /// while keeping the sealed hierarchy instantiable only via its subtypes.
  factory ApiException(
    int? statusCode,
    String? message, {
    String? endpoint,
    String? method,
    String? requestId,
    DateTime? timestamp,
    int retryCount = 0,
    String? rawBody,
    String? errorCode,
    Map<String, dynamic>? details,
    int? retryAfter,
  }) = _ApiExceptionImpl;

  factory ApiException.badRequest(String message,
          {String? endpoint,
          String? method,
          String? rawBody,
          String? errorCode,
          Map<String, dynamic>? details,
          String? requestId}) =>
      ApiException(400, message,
          endpoint: endpoint,
          method: method,
          rawBody: rawBody,
          errorCode: errorCode,
          details: details,
          requestId: requestId);

  factory ApiException.unauthorized(String message,
          {String? endpoint,
          String? method,
          String? rawBody,
          String? errorCode,
          Map<String, dynamic>? details,
          String? requestId}) =>
      ApiException(401, message,
          endpoint: endpoint,
          method: method,
          rawBody: rawBody,
          errorCode: errorCode,
          details: details,
          requestId: requestId);

  factory ApiException.forbidden(String message,
          {String? endpoint,
          String? method,
          String? rawBody,
          String? errorCode,
          Map<String, dynamic>? details,
          String? requestId}) =>
      ApiException(403, message,
          endpoint: endpoint,
          method: method,
          rawBody: rawBody,
          errorCode: errorCode,
          details: details,
          requestId: requestId);

  factory ApiException.notFound(String message,
          {String? endpoint,
          String? method,
          String? rawBody,
          String? errorCode,
          Map<String, dynamic>? details,
          String? requestId}) =>
      ApiException(404, message,
          endpoint: endpoint,
          method: method,
          rawBody: rawBody,
          errorCode: errorCode,
          details: details,
          requestId: requestId);

  factory ApiException.tooManyRequests(String message,
          {String? endpoint,
          String? method,
          String? rawBody,
          String? errorCode,
          Map<String, dynamic>? details,
          String? requestId}) =>
      ApiException(429, message,
          endpoint: endpoint,
          method: method,
          rawBody: rawBody,
          errorCode: errorCode,
          details: details,
          requestId: requestId);

  factory ApiException.internalServerError(String message,
          {String? endpoint,
          String? method,
          String? rawBody,
          String? errorCode,
          Map<String, dynamic>? details,
          String? requestId}) =>
      ApiException(500, message,
          endpoint: endpoint,
          method: method,
          rawBody: rawBody,
          errorCode: errorCode,
          details: details,
          requestId: requestId);

  factory ApiException.fromResponse(int statusCode, dynamic body,
      {String? endpoint, String? method, String? requestId}) {
    String message = '';
    String? code;
    Map<String, dynamic>? meta;
    String? raw;
    if (body is String) {
      raw = body;
      message = body;
    } else if (body is Map<String, dynamic>) {
      raw = body.toString();
      message = body['message']?.toString() ??
          body['error']?.toString() ??
          body['detail']?.toString() ??
          '';
      code = body['code']?.toString() ??
          body['errorCode']?.toString() ??
          body['error_code']?.toString();
      meta = body;
    }
    return ApiException(statusCode, message,
        endpoint: endpoint,
        method: method,
        requestId: requestId,
        rawBody: raw,
        errorCode: code,
        details: meta);
  }

  static String _generateRequestId() {
    final micros = DateTime.now().microsecondsSinceEpoch;
    return 'req_${micros}_${(micros * 1000) % 1000000}';
  }

  /// The error category for [statusCode].
  ApiErrorCode get code => ApiErrorCode.fromStatusCode(statusCode);

  /// Returns a user-safe message based on HTTP status code.
  /// Never exposes raw server messages or internal details.
  String get userMessage {
    switch (statusCode) {
      case 400:
        return 'Invalid request. Please check your input.';
      case 401:
        return 'Session expired. Please log in again.';
      case 403:
        return 'You do not have permission to perform this action.';
      case 404:
        return 'The requested resource was not found.';
      case 408:
        return 'The request timed out. Please try again.';
      case 429:
        return 'Too many requests. Please wait and try again.';
      case 500:
        return 'Service unavailable. Please try again later.';
      case 502:
        return 'Service unavailable. Please try again later.';
      case 503:
        return 'Service unavailable. Please try again later.';
      default:
        return 'An unexpected error occurred. Please try again.';
    }
  }

  String get message => _message;

  /// Structured JSON log record carrying correlation ID ([requestId]) and the
  /// full request context (endpoint, method, timestamp, status, retry count)
  /// so log aggregation can trace an incident back to the exact request.
  ///
  /// Note: this is the SINGLE definition of [toLogJson]. A previous duplicate
  /// `Map`-returning overload conflicted with the `String` version below and
  /// silently broke [toLogJsonString]; only the JSON-string form remains.
  Map<String, dynamic> toLogJsonMap() {
    final sanitized = sanitizeForLogging();
    return <String, dynamic>{
      'timestamp': sanitized.timestamp.toIso8601String(),
      'level': 'error',
      'type': 'ApiException',
      'requestId': sanitized.requestId,
      'endpoint': sanitized.endpoint,
      'method': sanitized.method,
      'statusCode': sanitized.statusCode,
      'retryCount': sanitized.retryCount,
      'retryAfter': sanitized.retryAfter,
      'errorCode': sanitized.errorCode,
      'message': sanitized.message,
      'details': sanitized.details,
    };
  }

  /// Single-line JSON rendering of [toLogJsonMap] for structured log sinks.
  String toLogJsonString() => jsonEncode(toLogJsonMap());

  /// Returns a redacted copy of this exception safe for logging.
  ApiException sanitizeForLogging() {
    final sanitizedMessage = _sanitizeMessage(_message);
    return ApiException(
      statusCode,
      sanitizedMessage,
      endpoint: endpoint,
      method: method,
      requestId: requestId,
      timestamp: timestamp,
      retryCount: retryCount,
      rawBody: rawBody != null ? _sanitizeMessage(rawBody!) : null,
      errorCode: errorCode,
      details: details,
      retryAfter: retryAfter,
    );
  }

  /// Redacts sensitive patterns from a message string.
  /// Truncates long messages, strips non-printable characters, and obfuscates
  /// potential PII (emails, phone numbers, tokens) to reduce leakage.
  String _sanitizeMessage(String raw) {
    if (raw.isEmpty) return raw;
    // Strip control characters except newline and tab
    String cleaned =
        raw.replaceAll(RegExp(r'[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]'), '');
    // Redact Bearer tokens to prevent auth token leakage
    cleaned = cleaned.replaceAll(
