@0xb9e8b85c3a7e62e3;

struct Pack {
    whiteOccupancy @0 :UInt64;
    blackOccupancy @1 :UInt64;
    walls @2 :UInt64;
    wdl @3 :Int8;
    rolloutDistribution @4 :List(UInt16);
}