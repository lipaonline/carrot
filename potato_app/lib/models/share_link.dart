import 'package:flutter/foundation.dart';

/// Public URL of the deployed web app, used to build scannable QR links on
/// native platforms (where there is no browser origin to derive it from).
/// Inject it at build time: --dart-define=WEB_APP_URL=https://your-app-url
const _webAppUrl = String.fromEnvironment('WEB_APP_URL');

String _appBase() {
  // On web, use the current origin so links always point to this deployment.
  if (kIsWeb) return Uri.base.origin;
  return _webAppUrl.replaceAll(RegExp(r'/+$'), '');
}

/// Value encoded in a share QR code. When the web app URL is known we produce a
/// scannable link (so a phone's native camera opens the app with the code
/// prefilled); otherwise we fall back to the raw code.
String buildShareValue(String code) {
  final base = _appBase();
  if (base.isEmpty) return code;
  return '$base/?code=$code';
}

/// Extract a room code from a scanned QR value, accepting either a raw code or
/// a share link of the form `<base>/?code=CODE`. Codes are always uppercase
/// alphanumeric, so normalizing the case is safe.
String extractCode(String scanned) {
  final value = scanned.trim();
  final fromQuery = Uri.tryParse(value)?.queryParameters['code'];
  if (fromQuery != null && fromQuery.isNotEmpty) {
    return fromQuery.toUpperCase();
  }
  return value.toUpperCase();
}
