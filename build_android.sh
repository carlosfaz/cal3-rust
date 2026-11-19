#!/usr/bin/env bash
# Compila cal3-rust como APK de Android (arm64 y armv7) con cargo-apk.
#
# Requisitos (una sola vez):
#   rustup target add aarch64-linux-android armv7-linux-androideabi
#   cargo install cargo-apk --locked
#   Android SDK (build-tools, platform 33) y NDK en ~/Android/sdk
#
# Uso:  ./build_android.sh        -> deja el APK en ./cal3-rust.apk
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")"

export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/sdk}"
NDK="$(ls -d "$ANDROID_HOME"/ndk/* | sort -V | tail -1)"
export ANDROID_NDK_ROOT="$NDK" ANDROID_NDK_HOME="$NDK"

# Firma con la clave de depuración de Android (suficiente para uso personal).
KS="$HOME/.android/debug.keystore"
if [ ! -f "$KS" ]; then
    mkdir -p "$HOME/.android"
    keytool -genkeypair -keystore "$KS" -storepass android -alias androiddebugkey \
        -keypass android -keyalg RSA -keysize 2048 -validity 10000 \
        -dname "CN=Android Debug,O=Android,C=US"
fi
export CARGO_APK_RELEASE_KEYSTORE="$KS" CARGO_APK_RELEASE_KEYSTORE_PASSWORD=android

cargo apk build --release --lib
cp target/release/apk/cal3-rust.apk ./cal3-rust.apk
echo "APK listo: $(pwd)/cal3-rust.apk"
