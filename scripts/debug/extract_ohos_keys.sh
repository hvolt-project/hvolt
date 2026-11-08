#!/bin/bash
# Extract OpenHarmony signing keys from OpenHarmony.p12
set -e

P12="/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/toolchains/lib/OpenHarmony.p12"
PASS="123456"
JAVA="/Applications/DevEco-Studio.app/Contents/jbr/Contents/Home/bin"
OUTDIR="/tmp/ohos_keys"
rm -rf "$OUTDIR"
mkdir -p "$OUTDIR"

echo "=== Extracting certificates ==="

# Export all certs as PEM
for alias in "openharmony application root ca" "openharmony application ca" "openharmony application profile debug" "openharmony application profile release" "openharmony application release"; do
    safe_name=$(echo "$alias" | tr ' ' '_')
    echo "  Exporting cert: $alias"
    "$JAVA/keytool" -exportcert -alias "$alias" -keystore "$P12" -storetype PKCS12 -storepass "$PASS" -file "$OUTDIR/${safe_name}.cer" -rfc
done

echo "=== Extracting private keys via openssl ==="

# Extract private keys (PEM) via openssl pkcs12
openssl pkcs12 -in "$P12" -passin "pass:$PASS" -nocerts -nodes -out "$OUTDIR/all_keys.pem" 2>/dev/null

# Also extract individual keys by alias
for alias in "openharmony application root ca" "openharmony application ca" "openharmony application profile debug" "openharmony application profile release" "openharmony application release"; do
    safe_name=$(echo "$alias" | tr ' ' '_')
    echo "  Extracting key: $alias"
    # Extract key pair for this alias
    openssl pkcs12 -in "$P12" -passin "pass:$PASS" -nocerts -nodes -name "$alias" 2>/dev/null | openssl ec -out "$OUTDIR/${safe_name}_key.pem" 2>/dev/null || echo "    (no key for $alias, may be CA cert only)"
done

echo "=== Files in $OUTDIR ==="
ls -la "$OUTDIR"

echo "=== Creating cert chain for app signing ==="
# App signing chain: app cert -> intermediate CA -> root CA
cat "$OUTDIR/openharmony_application_release.cer" "$OUTDIR/openharmony_application_ca.cer" "$OUTDIR/openharmony_application_root_ca.cer" > "$OUTDIR/app_cert_chain.pem"
echo "  Cert chain: $OUTDIR/app_cert_chain.pem"

echo "Done!"
