# Android app

A minimal Kotlin + Gradle Android app scaffolded by LingCode.

## First-time setup

This template ships without the Gradle wrapper binary. Generate it once
(requires a system Gradle, or Android Studio's bundled one):

    gradle wrapper

## Build & run with LingCode

Use the **Android** menu in LingCode:

- **Check Android Toolchain** — verify SDK / JDK / adb
- **Build Debug APK** — `gradlew assembleDebug`
- **Run on Device** — install + launch on a connected device/emulator

You'll need the Android SDK installed and `local.properties` pointing at it
(`sdk.dir=/path/to/Android/Sdk`), or set `ANDROID_HOME`.
