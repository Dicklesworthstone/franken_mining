/**
 * The fleet: procedural mining equipment and site pieces for the FrankenMining scenes. Pure factories of
 * Object3D and materials (no renderer, nothing touches the DOM at import or build time). Metres, y up,
 * +z forward for vehicles, origin on the ground under the machine's centre. Machines return
 * `{ group, state, set(partial), update(dt), lenses }`; `set` poses them, `update` advances wheels,
 * belts and spinning parts. Same-colour machines share geometry and materials.
 */
export {
  bake,
  COLOURS,
  grime,
  heapMaterial,
  lenses,
  makeRocks,
  makeTrack,
  mats,
  paint,
  ROCK_PALETTES,
  rockGeometry,
  rockMaterial,
  SITE,
  stats,
  type Grime,
  type Lenses,
  type RockPalette,
  type RockSet,
  type Track,
} from "./fleet-kit";
export { HAUL, makeHaulTruck, type HaulTruck, type HaulTruckOptions, type HaulTruckState } from "./fleet-truck";
export {
  FACE_SHOVEL,
  makeFaceShovel,
  makeRopeShovel,
  makeTrailingCable,
  ROPE_SHOVEL,
  type FaceShovel,
  type FaceShovelState,
  type RopeShovel,
  type RopeShovelOptions,
  type RopeShovelState,
} from "./fleet-shovel";
export {
  DOZER,
  DRILL,
  LOADER,
  makeDozer,
  makeDrillRig,
  makeWheelLoader,
  type Dozer,
  type DozerState,
  type DrillRig,
  type DrillRigState,
  type WheelLoader,
  type WheelLoaderState,
} from "./fleet-mobile";
export { makeJumbo, makeLHD, type Jumbo, type JumboState, type LHD, type LHDState } from "./fleet-underground";
export { makeLightTower, makePickup, type LightTower, type LightTowerState, type Pickup, type PickupState } from "./fleet-light";
export { makeConveyor, makeCrusher, type Conveyor, type ConveyorOptions, type ConveyorState, type Crusher, type CrusherOptions } from "./fleet-plant";
export { makeBoulders, makeMuckPile, makeStockpile, type MuckPileOptions, type Pile, type PileOptions } from "./fleet-rocks";
