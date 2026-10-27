const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const test = require('node:test');

function harness() {
  class Event {
    constructor(type, options = {}) { Object.assign(this, options); this.type = type; this.isTrusted = options.isTrusted ?? false; }
    preventDefault() { this.prevented = true; }
    stopImmediatePropagation() { this.stopped = true; }
  }
  const listeners = new Map(), delivered = [], reports = [], windowListeners = new Map();
  let time = 0;
  const canvas = {
    focus() {},
    addEventListener(type, fn) { const list = listeners.get(type) || []; list.push(fn); listeners.set(type,list); },
    dispatchEvent(event) {
      for (const fn of listeners.get(event.type) || []) { fn(event); if (event.stopped) break; }
      if (!event.stopped) delivered.push(event);
    }
  };
  const aligned = [], inputSources = [];
  let modelHit = () => true;
  const window = {addEventListener(type, fn) { windowListeners.set(type,fn); }, solidcraftRequirePencil:true,
    wasmBindings:{touch_set_pencil:value => inputSources.push(value)},
    solidcraftIsModelPoint:p => modelHit(p), solidcraftAlignFace:p => aligned.push(p)};
  vm.runInNewContext(fs.readFileSync(require('node:path').join(__dirname,'../SolidcraftForIPad/input.js'),'utf8'),
    {window,MouseEvent:Event,PointerEvent:Event,WheelEvent:Event,Map,Math});
  window.installSolidcraftInput(canvas, report => reports.push(report));
  const input = (type, id, x, y, pointerType = 'touch') => canvas.dispatchEvent(new Event(type,
    {isTrusted:true,pointerType,pointerId:id,clientX:x,clientY:y,button:0,timeStamp:time}));
  return {inputSources,hover:window.solidcraftPencilHover,input,delivered,reports,aligned,setModelHit:fn => { modelHit = fn; },advance:ms => { time += ms; },blur:() => windowListeners.get('blur')()};
}

test('one finger orbits without any primary-button selection', () => {
  const h=harness();
  h.input('pointerdown',1,500,400); h.advance(120); h.input('pointermove',1,550,430); h.input('pointerup',1,550,430);
  const buttons=h.delivered.filter(e=>e.type === 'pointerdown' || e.type === 'pointerup');
  assert.equal(buttons.filter(e=>e.type === 'pointerdown').length,1);
  assert.ok(buttons.every(e=>e.button === 1 && e.shiftKey));
  assert.ok(h.reports[0].text.includes('orbit: 1; pan: 0;'));
});

test('two fingers translate the centroid as pan without selecting', () => {
  const h=harness();
  h.input('pointerdown',1,400,400); h.input('pointerdown',2,600,400);
  // Move perpendicular to their separation; this avoids distance-change zoom noise.
  h.input('pointermove',1,400,420); h.input('pointermove',2,600,420);
  h.input('pointerup',1,400,420);
  const count=h.delivered.length;
  h.input('pointermove',2,650,450);
  assert.equal(h.delivered.length,count,'remaining finger must not suddenly orbit');
  h.input('pointerup',2,650,450);
  const pan=h.delivered.filter(e=>e.type === 'mousemove' && e.buttons === 4 && !e.shiftKey);
  assert.equal(pan.length,2);
  assert.equal(pan.at(-1).clientX,500); assert.equal(pan.at(-1).clientY,420);
  assert.ok(!h.delivered.some(e=>e.type === 'pointerdown' && e.button === 0));
});

test('pinch emits zoom while preserving pan routing', () => {
  const h=harness();
  h.input('pointerdown',1,400,400); h.input('pointerdown',2,600,400);
  h.input('pointermove',2,650,400);
  assert.ok(h.delivered.some(e=>e.type === 'wheel' && e.deltaY < 0));
  assert.ok(h.delivered.some(e=>e.type === 'mousemove' && e.buttons === 4 && !e.shiftKey));
});

test('Pencil supplies primary selection and ignores concurrent palm/finger input', () => {
  const h=harness();
  h.input('pointerdown',1,400,400);
  h.input('pointerdown',9,510,410,'pen');
  const count=h.delivered.length;
  h.input('pointerdown',2,700,500); h.input('pointermove',2,710,510);
  assert.equal(h.delivered.length,count);
  h.input('pointermove',9,520,420,'pen'); h.input('pointerup',9,520,420,'pen');
  const primary=h.delivered.filter(e=>e.type === 'pointerdown' && e.button === 0);
  assert.equal(primary.length,1); assert.equal(primary[0].shiftKey,false);
  assert.ok(h.delivered.some(e=>e.type === 'mousemove' && e.buttons === 1));
  assert.ok(h.reports.at(-1).text.includes('Pencil strokes: 1'));
});

test('cancel and focus loss release every held button', () => {
  const h=harness();
  h.input('pointerdown',1,400,400); h.advance(120); h.input('pointermove',1,420,420); h.input('pointercancel',1,420,420);
  assert.equal(h.delivered.at(-1).type,'pointerup');
  h.input('pointerdown',9,500,500,'pen'); h.blur();
  assert.equal(h.delivered.at(-1).type,'pointerup'); assert.equal(h.delivered.at(-1).button,0);
});

test('staggered two-finger landing with early movement never starts orbit', () => {
  const h=harness();
  h.input('pointerdown',1,400,400); h.advance(25);
  h.input('pointermove',1,409,406); h.advance(25);
  h.input('pointerdown',2,600,400);
  h.input('pointermove',1,410,410); h.input('pointermove',2,601,410);
  assert.ok(!h.delivered.some(e=>e.type === 'pointerdown' && e.shiftKey));
  assert.ok(h.delivered.some(e=>e.type === 'pointerdown' && e.button === 1 && !e.shiftKey));
});

test('a finger tap or sub-threshold resting jitter never rotates', () => {
  const h=harness();
  h.input('pointerdown',1,400,400); h.advance(200);
  h.input('pointermove',1,402,402); h.input('pointerup',1,402,402);
  assert.ok(!h.delivered.some(e=>e.type === 'pointerdown'));
});

test('cursor relocation is flushed with the button up before pan starts', () => {
  const h=harness();
  h.input('pointerdown',1,400,400); h.input('pointerdown',2,600,400);
  const press=h.delivered.findIndex(e=>e.type === 'pointerdown');
  assert.equal(h.delivered[press-1].type,'pointerup');
  assert.equal(h.delivered[press-1].buttons,0);
  assert.equal(h.delivered[press-1].clientX,500);
  assert.equal(h.delivered[press-2].type,'mousemove');
});

test('mouse clicks reach model selection without Pencil accumulation', () => {
  const h=harness();
  h.input('pointerdown',1,500,500,'mouse'); h.input('pointerup',1,500,500,'mouse');
  assert.equal(h.delivered.length,2);
  assert.deepEqual(h.inputSources,[false]);
});

test('finger taps and drags in interface regions use primary input', () => {
  const h=harness(); h.setModelHit(p=>p.clientX > 260);
  h.input('pointerdown',1,250,400); h.input('pointermove',1,350,400); h.input('pointerup',1,350,400);
  assert.ok(h.delivered.some(e=>e.type === 'pointerdown' && e.button === 0));
  assert.ok(h.delivered.some(e=>e.type === 'mousemove' && e.buttons === 1 && e.clientX === 350));
  assert.ok(!h.delivered.some(e=>e.type === 'pointerdown' && e.button === 1));
});

test('double finger tap aligns a face without emitting a selection click', () => {
  const h=harness();
  h.input('pointerdown',1,500,400); h.advance(60); h.input('pointerup',1,500,400);
  h.advance(120); h.input('pointerdown',2,502,401); h.advance(60); h.input('pointerup',2,502,401);
  assert.equal(h.aligned.length,1);
  assert.ok(!h.delivered.some(e=>e.type === 'pointerdown' && e.button === 0));
});

test('mouse interface clicks remain ordinary mouse input', () => {
  const h=harness(); h.setModelHit(p=>p.clientX > 260);
  h.input('pointerdown',1,100,50,'mouse'); h.input('pointerup',1,100,50,'mouse');
  assert.equal(h.delivered.length,2);
});


test('native Pencil hover moves the tooltip cursor without selecting and clears on exit', () => {
  const h = harness();
  h.hover(120, 80, false);
  assert.equal(h.delivered.at(-1).type, 'mousemove');
  assert.equal(h.delivered.at(-1).buttons, 0);
  assert.equal(h.delivered.at(-1).clientX, 120);
  h.hover(120, 80, true);
  assert.equal(h.delivered.at(-1).type, 'mouseleave');
  h.input('pointerdown', 1, 120, 80, 'pen');
  const count = h.delivered.length;
  h.hover(140, 90, false);
  assert.equal(h.delivered.length, count, 'hover must not interrupt a Pencil stroke');
});


test('Pencil tremor preserves the stationary tooltip cursor; movement and exit update it', () => {
  const h = harness();
  h.hover(120,80,false);
  const count = h.delivered.length;
  h.hover(121,81,false);
  h.input('pointermove', 7, 122, 81, 'pen');
  assert.equal(h.delivered.length,count);
  h.hover(130,80,false);
  assert.equal(h.delivered.at(-1).clientX,130);
  h.input('pointerleave', 7, 130, 80, 'pen');
  assert.equal(h.delivered.at(-1).type,'pointerleave');
  assert.ok(h.delivered.some(e => e.type === 'mouseleave'));
});


test('Pencil, mouse, and finger presses identify their source without synthetic Shift', () => {
  const h = harness();
  h.input('pointerdown',1,500,400,'pen');
  h.input('pointerup',1,500,400,'pen');
  h.input('pointerdown',2,500,400,'mouse');
  h.input('pointerup',2,500,400,'mouse');
  h.input('pointerdown',3,500,400,'touch');
  assert.deepEqual(h.inputSources,[true,false,false]);
  assert.ok(h.delivered.filter(e => e.type === 'pointerdown' && e.button === 0).every(e => !e.shiftKey));
});


test('ignored palm input cannot change an active Pencil selection into mouse selection', () => {
  const h = harness();
  h.input('pointerdown',1,500,400,'pen');
  h.input('pointerdown',2,520,420,'touch');
  h.input('pointerup',2,520,420,'touch');
  h.input('pointerup',1,500,400,'pen');
  assert.deepEqual(h.inputSources,[true]);
});
