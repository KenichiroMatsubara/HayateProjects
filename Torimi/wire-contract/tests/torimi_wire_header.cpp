#include "../../../Hayate/crates/platform/mobile/android/cpp/generated/torimi_wire.hpp"

using namespace std::literals;

static_assert(torimi::wire::kBundleRoute == "/bundle.js"sv);
static_assert(torimi::wire::kReloadMessage == "reload"sv);
static_assert(torimi::wire::kHayateHostGlobal == "__hayateHost"sv);
static_assert(torimi::wire::kTsubamePumpFrameProperty == "pumpFrame"sv);
static_assert(torimi::wire::kTsubameStopProperty == "stop"sv);

int main() { return 0; }
