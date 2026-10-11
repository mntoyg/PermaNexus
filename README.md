# PermaNexus

ฐานโปรเจกต์วันที่ 1: สัญญา gRPC ระหว่าง Hardware Actuator (C++) และ PermaCore (Rust)

```text
proto/                   Shared Protocol Buffers contracts
src/layer1_actuator/     C++ gRPC contract library
src/layer2_permacore/    Rust gRPC client contract library
src/layer3_omni_agent/   Go integration placeholder
src/layer4_telemetry_ai/ Python integration placeholder
```

## การ Build

ต้องมี GNU Make, CMake >= 3.24, C++17 compiler, Rust/Cargo, protoc >= 3.15 และ gRPC C++ พร้อม CMake packages ของ Protobuf/gRPC โดยใช้ compiler และ dependencies ที่เข้ากัน

หาก dependencies ไม่อยู่ใน path เริ่มต้น ให้ส่ง `CMAKE_FLAGS="-DCMAKE_PREFIX_PATH=..."` หรือ `CMAKE_FLAGS="-DCMAKE_TOOLCHAIN_FILE=..."` ให้ Make ส่วน Rust ใช้ `PROTOC` เพื่อระบุตำแหน่ง protoc ได้

```powershell
mingw32-make build
mingw32-make check
```

บน Linux/macOS ใช้ `make` แทน `mingw32-make` โดยมี `Cargo.lock` ในโปรเจกต์แล้วเพื่อให้ Build ใช้ dependency versions ชุดเดียวกัน

`proto` สร้าง C++ bindings และ descriptor ใน `build/actuator/generated/` ส่วน Rust สร้าง client bindings ใน Cargo OUT_DIR ระหว่าง Build ไม่ต้อง commit generated code

ผลลัพธ์ตอนนี้เป็นไลบรารีสัญญา ยังไม่มี gRPC server/client ที่รันจริง การเข้าถึง register, device allowlist, TLS และการยืนยันตัวตนต้องเชื่อมต่อในขั้น implementation ก่อนใช้งานกับฮาร์ดแวร์

การเขียน register ใช้ expected value ที่จำเป็นและต้องเป็น atomic compare-and-write ฝั่ง server ตามสัญญา หากฮาร์ดแวร์หรือ register ทำอย่างปลอดภัยไม่ได้ ให้คืน UNIMPLEMENTED; ห้ามใช้กับ register ที่อ่านแล้วเกิด side effects
