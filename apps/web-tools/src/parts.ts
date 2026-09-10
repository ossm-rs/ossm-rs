export type MaterialType = "printed" | "metal" | "rubber" | "electronics";

export type CatalogPart = {
  id: string;
  name: string;
  /** Markdown. Include STL/buy links inline as `[label](url)`. */
  description?: string;
  meshes: string[];
  parent?: string;
  offset?: [number, number, number];
  offsets?: Array<[number, number, number]>;
  /** Used by the GLB processing script: bake each source mesh's world transform
   *  into its primitive vertices and merge them into a single glTF mesh named
   *  after the part id. After running the script, this part's `meshes` array
   *  should be reduced to `["<id>"]`. */
  mergeMeshes?: boolean;
  /** Override the GLB-baked material with one of our standard presets. */
  materialType?: MaterialType;
};

export const PART_CATALOG: CatalogPart[] = [
  {
    id: "main-motor-clamp",
    name: "Main motor clamp",
    description: `3D-printed motor mount that secures the 57AIM30 to the housing with embedded M5 lock nuts.

**Files:**
- [Motor Mount – Main Clamp](https://github.com/jollydodo/OSSM-ALT-Edition/blob/main/Print-Files/Heavy%20MGN15/Strength%20Parts/Motor%20Mount%20-%20Main%20Clamp.stl)`,
    materialType: "printed",
    parent: "motor-57aim30",
    offset: [0.095, 0, 0],
    meshes: ["main-motor-clamp"],
  },
  {
    id: "m5-lock-nut",
    name: "M5 lock nut",
    description: `M5 nylon-insert lock nut for motor fastening (vibration-resistant).

**Sources:**
- [Toolstation – M5 lock nuts](https://www.toolstation.nl/borgmoeren/p12051)`,
    materialType: "metal",
    parent: "main-motor-clamp",
    offset: [0.06, 0, 0],
    meshes: [
      "m5-lock-nut-0",
      "m5-lock-nut-1",
      "m5-lock-nut-2",
      "m5-lock-nut-3",
    ],
  },
  {
    id: "m3-nut-clamp",
    name: "M3 nut",
    description: `M3 captive nut used in the belt clamp.

**Sources:**
- [Amazon.nl – M3 nut & bolt kit](https://www.amazon.nl/dp/B0B3MGZ7T2)`,
    materialType: "metal",
    parent: "main-motor-clamp",
    offsets: [
      [0, 0, -0.025],
      [0, 0, 0.025],
    ],
    meshes: ["m3-nut-clamp-0", "m3-nut-clamp-1"],
  },
  {
    id: "motor-57aim30",
    name: "57AIM30 motor",
    description: `57AIM30 integrated NEMA23 closed-loop stepper-servo motor (~€100 incl. EU shipping).`,
    materialType: "metal",
    meshes: ["motor-57aim30"],
  },
  {
    id: "rail-mgn15h-400",
    name: "MGN15H 400mm rail",
    description: `MGN15H linear rail, 400 mm length (Hiwin preferred, AliExpress alternatives acceptable).`,
    materialType: "metal",
    parent: "belt-mechanics-rear",
    offset: [-0.135, 0, 0],
    meshes: ["rail-mgn15h-400"],
  },
  {
    id: "motor-power-plug",
    name: "Motor power plug",
    description: `Phoenix-style screw terminal block for the motor's power leads. Typically included with the 57AIM30 motor.`,
    materialType: "electronics",
    parent: "motor-57aim30",
    offset: [0, -0.03, 0],
    meshes: ["motor-power-plug"],
  },
  {
    id: "mechanic-mount-top",
    name: "Mechanic mount top",
    description: `Upper half of the belt mechanics carriage that clamps onto the linear rail and houses bearings/dowels.

**Files:**
- [Belt Mechanics – Front (no branding)](https://github.com/jollydodo/OSSM-ALT-Edition/blob/main/Print-Files/Heavy%20MGN15/Strength%20Parts/Belt%20Mechanics%20-%20Front%20-%20Single%20Color%20-%20No%20Branding.stl)`,
    materialType: "printed",
    parent: "rail-mgn15h-400",
    offset: [-0.08, 0, 0],
    meshes: ["mechanic-mount-top"],
  },
  {
    id: "belt",
    name: "Belt",
    description: `GT2 timing belt (9, 12, or 15 mm wide; length = rail length + ~10 cm).`,
    materialType: "rubber",
    parent: "rail-mgn15h-400",
    offset: [-0.05, 0, 0],
    meshes: ["belt"],
  },
  {
    id: "precision-dowel-m5",
    name: "Precision dowel (M5)",
    description: `Precision M5 stainless steel dowel pin kit covering all bearings and clamp pins.

**Sources:**
- [Amazon.nl – M5 dowel pin kit](https://www.amazon.nl/dp/B0DR7R87KQ)`,
    materialType: "metal",
    parent: "belt-mechanics-rear",
    offset: [-0.065, 0, 0],
    meshes: ["precision-dowel-m5-0", "precision-dowel-m5-1"],
  },
  {
    id: "bearing-625",
    name: "625 bearing",
    description: `625-RS sealed ball bearing used as belt-guide bearings (6 required).

**Sources:**
- [Amazon.nl – 625-RS bearings](https://www.amazon.nl/dp/B071RXC7FV)`,
    materialType: "metal",
    parent: "precision-dowel-m5",
    offset: [-0.04, 0, 0],
    meshes: [
      "bearing-625-0",
      "bearing-625-1",
      "bearing-625-2",
      "bearing-625-3",
      "bearing-625-4",
      "bearing-625-5",
    ],
  },
  {
    id: "top-cover",
    name: "Top cover",
    description: `Decorative cover over the PCB stack on the motor mount. Pick one of the solid covers, optionally paired with the transparent LED window insert.

**Files:**
- [Top cover – no LED hole](https://github.com/jollydodo/OSSM-ALT-Edition/blob/main/Print-Files/Heavy%20MGN15/Decorative%20Parts/Motor%20Mount%20-%20Top%20Cover%20-%20Single%20Color%20-%20No%20Branding%20-%20No%20LED.stl)
- [Top cover – with LED hole](https://github.com/jollydodo/OSSM-ALT-Edition/blob/main/Print-Files/Heavy%20MGN15/Decorative%20Parts/Motor%20Mount%20-%20Top%20Cover%20-%20Single%20Color%20-%20No%20Branding.stl)
- [Transparent LED cover insert](https://github.com/jollydodo/OSSM-ALT-Edition/blob/main/Print-Files/Heavy%20MGN15/Decorative%20Parts/Motor%20Mount%20-%20Transparant%20LED%20Cover.stl)`,
    materialType: "printed",
    mergeMeshes: true,
    parent: "pcb-holder",
    offset: [0.09, 0, 0],
    meshes: ["top-cover"],
  },
  {
    id: "motor-cover-spacer",
    name: "Motor cover spacer",
    description: `Decorative spacer ring that sits on top of the motor mount and holds captive M3 nuts.

**Files:**
- [Motor Mount – Spacer Ring](https://github.com/jollydodo/OSSM-ALT-Edition/blob/main/Print-Files/Heavy%20MGN15/Decorative%20Parts/Motor%20Mount%20-%20Spacer%20Ring.stl)`,
    materialType: "printed",
    parent: "main-motor-clamp",
    offset: [0.055, 0, 0],
    meshes: ["motor-cover-spacer"],
  },
  {
    id: "belt-mechanics-rear",
    name: "Belt mechanics rear",
    description: `Rear half of the belt mechanics carriage that mates to the rail and front piece via dowel pins.

**Files:**
- [Belt Mechanics – Rear](https://github.com/jollydodo/OSSM-ALT-Edition/blob/main/Print-Files/Heavy%20MGN15/Strength%20Parts/Belt%20Mechanics%20-%20Rear.stl)`,
    materialType: "printed",
    parent: "motor-57aim30",
    offset: [-0.08, 0, 0],
    meshes: ["belt-mechanics-rear"],
  },
  {
    id: "pcb-holder",
    name: "PCB holder",
    description: `Mount that secures the main PCB (and optional brake chopper PCB) to the motor housing.

**Files:**
- [Motor Mount – PCB Holder](https://github.com/jollydodo/OSSM-ALT-Edition/blob/main/Print-Files/Heavy%20MGN15/Decorative%20Parts/Motor%20Mount%20-%20PCB%20Holder.stl)`,
    materialType: "printed",
    parent: "motor-cover-spacer",
    offset: [0.035, 0, 0],
    meshes: ["pcb-holder"],
  },
  {
    id: "front-thread",
    name: "Front thread",
    description: `Front M24 toy-mount thread that screws onto the belt clamp for attaching accessories.

**Files:**
- [Toy Mount Thread](https://github.com/jollydodo/OSSM-ALT-Edition/blob/main/Print-Files/Heavy%20MGN15/Strength%20Parts/Toy%20Mount%20Thread.stl)`,
    materialType: "printed",
    parent: "rail-mgn15h-400",
    offset: [0, 0, 0.05],
    meshes: ["front-thread"],
  },
  {
    id: "front-belt-tensioner",
    name: "Front belt tensioner",
    description: `Front belt tensioner block used to pinch and tension the GT2 belt at the carriage.

**Files:**
- [Belt Tensioner](https://github.com/jollydodo/OSSM-ALT-Edition/blob/main/Print-Files/Heavy%20MGN15/Strength%20Parts/Belt%20Tensioner.stl)`,
    materialType: "printed",
    parent: "front-clamp",
    offset: [0, 0, 0.025],
    meshes: ["front-belt-tensioner"],
  },
  {
    id: "front-clamp",
    name: "Front clamp",
    description: `Front belt clamp half that sandwiches the belt against the front thread.

**Files:**
- [Belt Clamp (x2)](https://github.com/jollydodo/OSSM-ALT-Edition/blob/main/Print-Files/Heavy%20MGN15/Strength%20Parts/Belt%20Clamp%20(x2).stl)`,
    materialType: "printed",
    parent: "front-thread",
    offset: [0, 0, 0.065],
    meshes: ["front-clamp"],
  },
  {
    id: "rear-tensioner-cover",
    name: "Rear tensioner cover",
    description: `Cover piece over the rear belt tensioner assembly.

**Files:**
- [Belt Tension Cover](https://github.com/jollydodo/OSSM-ALT-Edition/blob/main/Print-Files/Heavy%20MGN15/Decorative%20Parts/Belt%20Tension%20Cover.stl)`,
    materialType: "printed",
    parent: "rear-bolt-m5x16",
    offset: [0, 0, -0.04],
    meshes: ["rear-tensioner-cover"],
  },
  {
    id: "back-belt-tensioner",
    name: "Back belt tensioner",
    description: `Rear belt tensioner block, identical print to the front tensioner, used at the back of the belt loop.

**Files:**
- [Belt Tensioner](https://github.com/jollydodo/OSSM-ALT-Edition/blob/main/Print-Files/Heavy%20MGN15/Strength%20Parts/Belt%20Tensioner.stl)`,
    materialType: "printed",
    parent: "rear-nut-clamp",
    offset: [0, 0, -0.045],
    meshes: ["back-belt-tensioner"],
  },
  {
    id: "rear-thread",
    name: "Rear thread",
    description: `Rear-side clamp body that pairs with the front thread to form the M24 mount/belt clamp.

**Files:**
- [Belt Clamp (x2)](https://github.com/jollydodo/OSSM-ALT-Edition/blob/main/Print-Files/Heavy%20MGN15/Strength%20Parts/Belt%20Clamp%20(x2).stl)`,
    materialType: "printed",
    parent: "rail-mgn15h-400",
    offset: [0, 0, -0.06],
    meshes: ["rear-thread"],
  },
  {
    id: "rear-nut-clamp",
    name: "Rear nut clamp",
    description: `Printed captive nut clamp used at the rear of the belt clamp assembly.

**Files:**
- [Nut (x2)](https://github.com/jollydodo/OSSM-ALT-Edition/blob/main/Print-Files/Heavy%20MGN15/Strength%20Parts/Nut%20(x2).stl)`,
    materialType: "printed",
    parent: "rear-thread",
    offset: [0, 0, -0.06],
    meshes: ["rear-nut-clamp"],
  },
  {
    id: "motor-pulley",
    name: "Motor pulley (GT2 20T)",
    description: `GT2 20-tooth pulley sized for up to 15 mm belt width.`,
    materialType: "metal",
    parent: "motor-57aim30",
    offset: [-0.035, 0, 0],
    meshes: ["motor-pulley"],
  },
  {
    id: "brake-chopper",
    name: "Brake chopper",
    description: `Optional add-on PCB that dissipates back-EMF spikes (12W continuous, 360W peak, 33V threshold).

**Files:**
- [PCB design on GitHub](https://github.com/jollydodo/OSSM-ALT-Edition/tree/main/PCB-Design)`,
    materialType: "electronics",
    mergeMeshes: true,
    parent: "motor-cover-spacer",
    offset: [0.05, 0, 0],
    meshes: ["brake-chopper"],
  },
  {
    id: "rear-bolt-m5x16",
    name: "Rear bolt (M5×16)",
    description: `M5x16 socket-head cap bolt (from M5 bolt kit).

**Sources:**
- [Amazon.nl – M5 nut & bolt kit](https://www.amazon.nl/dp/B0DPWYNDZF)`,
    materialType: "metal",
    parent: "back-belt-tensioner",
    offset: [0, 0, -0.03],
    meshes: ["rear-bolt-m5x16"],
  },
  {
    id: "back-belt-tensioner-nut",
    name: "Back belt tensioner nut",
    description: `M5 captive nut for the rear belt tensioner.

**Sources:**
- [Amazon.nl – M5 nut & bolt kit](https://www.amazon.nl/dp/B0DPWYNDZF)`,
    materialType: "metal",
    parent: "rear-thread",
    offset: [0, 0, 0.045],
    meshes: ["back-belt-tensioner-nut"],
  },
  {
    id: "front-bolt-m3x16",
    name: "Front bolt (M3×16)",
    description: `M3x16 socket-head cap bolt for the optional front belt clamp fastening.

**Sources:**
- [Amazon.nl – M3 nut & bolt kit](https://www.amazon.nl/dp/B0B3MGZ7T2)`,
    materialType: "metal",
    parent: "front-thread",
    offset: [0, -0.035, 0],
    meshes: ["front-bolt-m3x16"],
  },
  {
    id: "front-belt-tensioner-nut",
    name: "Front belt tensioner nut",
    description: `M3 captive nut for the front belt tensioner clamp.

**Sources:**
- [Amazon.nl – M3 nut & bolt kit](https://www.amazon.nl/dp/B0B3MGZ7T2)`,
    materialType: "metal",
    parent: "front-thread",
    offset: [0, 0.02, 0],
    meshes: ["front-belt-tensioner-nut"],
  },
  {
    id: "carriage-m3-bolts-rail",
    name: "Carriage M3×0.5 bolts (rail side)",
    description: `M3x12 socket-head bolts attaching the carriage to the MGN15H rail block (4 pcs).

**Sources:**
- [Amazon.nl – M3 nut & bolt kit](https://www.amazon.nl/dp/B0B3MGZ7T2)`,
    materialType: "metal",
    parent: "mechanic-mount-top",
    offset: [0, 0.025, 0],
    meshes: ["carriage-m3-bolts-rail-0", "carriage-m3-bolts-rail-1"],
  },
  {
    id: "carriage-m3-bolts-motor",
    name: "Carriage M3×0.5 bolts (motor side)",
    description: `M3 bolts joining the belt-mechanics carriage halves (from M3 kit).

**Sources:**
- [Amazon.nl – M3 nut & bolt kit](https://www.amazon.nl/dp/B0B3MGZ7T2)`,
    materialType: "metal",
    parent: "belt-mechanics-rear",
    offset: [0, 0.05, 0],
    meshes: ["carriage-m3-bolts-motor-0", "carriage-m3-bolts-motor-1"],
  },
  {
    id: "pcb-bolts",
    name: "PCB M3 bolts",
    description: `M3x8 socket-head bolts mounting the PCB to the printed holder (2 pcs).

**Sources:**
- [Amazon.nl – M3 nut & bolt kit](https://www.amazon.nl/dp/B0B3MGZ7T2)`,
    materialType: "metal",
    parent: "main-pcb",
    offset: [0.045, 0, 0],
    meshes: ["pcb-bolts-0", "pcb-bolts-1"],
  },
  {
    id: "pcb-nuts",
    name: "PCB M3 nuts",
    description: `M3 captive nuts in the PCB holder.

**Sources:**
- [Amazon.nl – M3 nut & bolt kit](https://www.amazon.nl/dp/B0B3MGZ7T2)`,
    materialType: "metal",
    parent: "pcb-holder",
    offset: [0.05, 0, 0],
    meshes: ["pcb-nuts-0", "pcb-nuts-1"],
  },
  {
    id: "rail-case-nuts",
    name: "Rail case nuts",
    description: `M5 captive nuts seated in the motor housing for rail/case bolts.

**Sources:**
- [Amazon.nl – M5 nut & bolt kit](https://www.amazon.nl/dp/B0DPWYNDZF)`,
    materialType: "metal",
    parent: "belt-mechanics-rear",
    offsets: [
      [0, 0.025, 0],
      [0, 0.025, 0],
      [0.025, 0, 0],
    ],
    meshes: ["rail-case-nuts-0", "rail-case-nuts-1", "rail-case-nuts-2"],
  },
  {
    id: "rail-case-bolts",
    name: "Rail case bolts",
    description: `M5x30 socket-head bolts fastening the motor housing/case to the rail end.

**Sources:**
- [Amazon.nl – M5 nut & bolt kit](https://www.amazon.nl/dp/B0DPWYNDZF)`,
    materialType: "metal",
    parent: "mechanic-mount-top",
    offset: [-0.05, 0, 0],
    meshes: ["rail-case-bolts-0", "rail-case-bolts-1", "rail-case-bolts-2"],
  },
  {
    id: "clamp-bolts",
    name: "Clamp bolts",
    description: `M5x30 (x2) and M5x20 (x1) socket-head bolts for the front mount/belt clamp.

**Sources:**
- [Amazon.nl – M5 nut & bolt kit](https://www.amazon.nl/dp/B0DPWYNDZF)`,
    materialType: "metal",
    parent: "belt-mechanics-rear",
    offset: [-0.05, 0, 0],
    meshes: [
      "clamp-bolts-0",
      "clamp-bolts-1",
      "clamp-bolts-2",
      "clamp-bolts-3",
    ],
  },
  {
    id: "pcb-holder-bolts",
    name: "PCB holder bolts",
    description: `M3x40 bolts that fasten the PCB holder through the housing (2 pcs).

**Sources:**
- [Amazon.nl – M3 nut & bolt kit](https://www.amazon.nl/dp/B0B3MGZ7T2)`,
    materialType: "metal",
    parent: "pcb-holder",
    offset: [0.05, 0, 0],
    meshes: ["pcb-holder-bolts-0", "pcb-holder-bolts-1"],
  },
  {
    id: "motor-cover-spacer-nut",
    name: "Motor cover spacer nut",
    description: `M3 captive nut pressed into the motor cover spacer ring.

**Sources:**
- [Amazon.nl – M3 nut & bolt kit](https://www.amazon.nl/dp/B0B3MGZ7T2)`,
    materialType: "metal",
    parent: "motor-cover-spacer",
    offset: [0, -0.05, 0],
    meshes: ["motor-cover-spacer-nut"],
  },
  {
    id: "top-cover-bolt",
    name: "Top cover bolt",
    description: `Single M3x12 socket-head bolt securing the top cover.

**Sources:**
- [Amazon.nl – M3 nut & bolt kit](https://www.amazon.nl/dp/B0B3MGZ7T2)`,
    materialType: "metal",
    parent: "top-cover",
    offset: [0.045, 0, 0],
    meshes: ["top-cover-bolt"],
  },
  {
    id: "main-pcb",
    name: "Main PCB",
    description: `Custom 28V USB-C PD ESP32-S3 controller PCB with RS485 and 140W power delivery.

**Files:**
- [PCB design on GitHub](https://github.com/jollydodo/OSSM-ALT-Edition/tree/main/PCB-Design)`,
    materialType: "electronics",
    mergeMeshes: true,
    parent: "pcb-holder",
    offset: [0.05, 0, 0],
    meshes: ["main-pcb"],
  },
];
