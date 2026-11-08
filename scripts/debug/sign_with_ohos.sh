#!/bin/bash
set -e

P12="/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/toolchains/lib/OpenHarmony.p12"
PASS="123456"
JAVA="/Applications/DevEco-Studio.app/Contents/jbr/Contents/Home/bin"
SIGN_TOOL="/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/toolchains/lib/hap-sign-tool.jar"
WORKDIR="/tmp/ohos_sign"
rm -rf "$WORKDIR"
mkdir -p "$WORKDIR"

echo "=== Step 1: Export certificate chain files ==="

# Profile signing cert chain: profile debug cert -> intermediate CA -> root CA
for alias in "openharmony application profile debug" "openharmony application ca" "openharmony application root ca"; do
    safe_name=$(echo "$alias" | tr ' ' '_')
    "$JAVA/keytool" -exportcert -alias "$alias" -keystore "$P12" -storetype PKCS12 -storepass "$PASS" -file "$WORKDIR/${safe_name}.pem" -rfc 2>/dev/null
done

# Profile cert chain (3-level, leaf first)
cat "$WORKDIR/openharmony_application_profile_debug.pem" \
    "$WORKDIR/openharmony_application_ca.pem" \
    "$WORKDIR/openharmony_application_root_ca.pem" \
    > "$WORKDIR/profile_cert_chain.pem"
echo "  Profile cert chain: $WORKDIR/profile_cert_chain.pem"

# App signing cert chain: app release cert -> intermediate CA -> root CA
"$JAVA/keytool" -exportcert -alias "openharmony application release" -keystore "$P12" -storetype PKCS12 -storepass "$PASS" -file "$WORKDIR/openharmony_application_release.pem" -rfc 2>/dev/null

cat "$WORKDIR/openharmony_application_release.pem" \
    "$WORKDIR/openharmony_application_ca.pem" \
    "$WORKDIR/openharmony_application_root_ca.pem" \
    > "$WORKDIR/app_cert_chain.pem"
echo "  App cert chain: $WORKDIR/app_cert_chain.pem"

echo "=== Step 2: Create profile JSON ==="

# Get device UDID (if connected)
UDID=$(hdc list targets 2>/dev/null | head -1 | awk '{print $1}' || echo "")
if [ -z "$UDID" ]; then
    UDID="69C7505BE341BDA5948C3C0CB44ABCD530296054159EFE0BD16A16CD0129CC42"
    echo "  No device connected, using default UDID"
fi
echo "  Device UDID: $UDID"

# Read app cert as single-line base64
APP_CERT_B64=$(sed '1d;$d' "$WORKDIR/openharmony_application_release.pem" | tr -d '\n')
echo "  App cert (base64): ${APP_CERT_B64:0:50}..."

BUNDLE_NAME="com.example.hvolt.app"
NOW=$(date +%s)
NOT_AFTER=$((NOW + 365*24*3600))

cat > "$WORKDIR/profile.json" << JSONEOF
{
    "version-name": "2.0.0",
    "version-code": 2,
    "uuid": "$(uuidgen 2>/dev/null || echo 'fe686e1b-3770-4824-a938-961b140a7c98')",
    "validity": {
        "not-before": $NOW,
        "not-after": $NOT_AFTER
    },
    "type": "debug",
    "bundle-info": {
        "developer-id": "OpenHarmony",
        "development-certificate": "-----BEGIN CERTIFICATE-----\n${APP_CERT_B64}\n-----END CERTIFICATE-----\n",
        "bundle-name": "$BUNDLE_NAME",
        "apl": "normal",
        "app-feature": "hos_normal_app"
    },
    "acls": {
        "allowed-acls": [""]
    },
    "permissions": {
        "restricted-permissions": [""]
    },
    "debug-info": {
        "device-ids": ["$UDID"],
        "device-id-type": "udid"
    },
    "issuer": "pki_internal"
}
JSONEOF
echo "  Profile JSON: $WORKDIR/profile.json"

echo "=== Step 3: Sign profile ==="

"$JAVA/java" -jar "$SIGN_TOOL" sign-profile \
    -mode localSign \
    -keyAlias "openharmony application profile debug" \
    -keyPwd "$PASS" \
    -profileCertFile "$WORKDIR/profile_cert_chain.pem" \
    -inFile "$WORKDIR/profile.json" \
    -signAlg SHA256withECDSA \
    -keystoreFile "$P12" \
    -keystorePwd "$PASS" \
    -outFile "$WORKDIR/debug.p7b" 2>&1

echo "=== Step 4: Sign HAP ==="

UNSIGNED_HAP="$1"
if [ -z "$UNSIGNED_HAP" ]; then
    # Try to find unsigned HAP
    UNSIGNED_HAP=$(ls ~/workspace/better-harmony-app-build/examples/test-app/build/src/build/default/outputs/default/*-unsigned.hap 2>/dev/null | head -1)
fi

if [ -z "$UNSIGNED_HAP" ] || [ ! -f "$UNSIGNED_HAP" ]; then
    echo "  No unsigned HAP found. Skip sign-app."
    echo "  To sign: sign_with_ohos.sh <path-to-unsigned.hap>"
    echo ""
    echo "=== Generated files in $WORKDIR ==="
    ls -la "$WORKDIR"
    exit 0
fi

echo "  Unsigned HAP: $UNSIGNED_HAP"
SIGNED_HAP="$WORKDIR/signed.hap"

"$JAVA/java" -jar "$SIGN_TOOL" sign-app \
    -mode localSign \
    -keyAlias "openharmony application release" \
    -keyPwd "$PASS" \
    -appCertFile "$WORKDIR/app_cert_chain.pem" \
    -profileFile "$WORKDIR/debug.p7b" \
    -profileSigned 1 \
    -inFile "$UNSIGNED_HAP" \
    -signAlg SHA256withECDSA \
    -keystoreFile "$P12" \
    -keystorePwd "$PASS" \
    -outFile "$SIGNED_HAP" 2>&1

echo "=== Done ==="
ls -la "$WORKDIR"
echo "Signed HAP: $SIGNED_HAP"
