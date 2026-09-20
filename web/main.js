// iterframes landing page. No dependencies. Both animations are built from
// the constants below, and every number printed next to them is computed from
// the same constants, so a drawing and its caption cannot disagree.

(function () {
  "use strict";

  var reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  /* ---------- copy buttons ---------- */

  document.querySelectorAll(".copy").forEach(function (button) {
    var state = button.querySelector(".copy-state");
    var idle = state.textContent;
    var timer;
    button.addEventListener("click", function () {
      if (!navigator.clipboard) {
        state.textContent = "Press ⌘C";
        return;
      }
      navigator.clipboard.writeText(button.dataset.copy).then(function () {
        state.textContent = "Copied";
        clearTimeout(timer);
        timer = setTimeout(function () { state.textContent = idle; }, 1800);
      }, function () {
        state.textContent = "Press ⌘C";
      });
    });
  });

  /* ---------- the two timelines ---------- */

  // One decode costs DECODE. The overlap benchmark measured per-frame work as
  // expensive as decoding, and the whole run came out 16% longer than decoding
  // the video alone, which is where WORK comes from. Both totals follow.
  var FRAMES = 12;
  var DECODE = 1;
  var WORK = (FRAMES * DECODE * 1.16 - DECODE) / FRAMES;
  var AXIS = 26;                       // units drawn across the full width
  var MS_PER_UNIT = 145;

  var trace = document.getElementById("trace");

  function planSerial() {
    var loop = [], done = [], at = 0;
    for (var n = 0; n < FRAMES; n++) {
      loop.push({ kind: "decode", x: at, w: DECODE });
      at += DECODE;
      loop.push({ kind: "work", x: at, w: WORK });
      at += WORK;
      done.push(at);
    }
    return { loop: loop, decoder: [], done: done, total: at };
  }

  function planOverlap() {
    var loop = [], decoder = [], done = [], n;
    for (n = 0; n < FRAMES; n++) {
      decoder.push({ kind: "decode", x: n * DECODE, w: DECODE });
    }
    var at = DECODE;                   // the loop starts on the first frame
    for (n = 0; n < FRAMES; n++) {
      loop.push({ kind: "work", x: at, w: WORK });
      at += WORK;
      done.push(at);
    }
    return { loop: loop, decoder: decoder, done: done, total: at };
  }

  var groups = [
    {
      plan: planSerial(),
      root: document.getElementById("group-serial"),
      lanes: [document.getElementById("lane-serial")],
      strip: document.getElementById("strip-serial"),
      time: document.getElementById("time-serial")
    },
    {
      plan: planOverlap(),
      root: document.getElementById("group-overlap"),
      lanes: [document.getElementById("lane-overlap"), document.getElementById("lane-decoder")],
      strip: document.getElementById("strip-overlap"),
      time: document.getElementById("time-overlap")
    }
  ];

  groups.forEach(function (group) {
    draw(group.lanes[0], group.plan.loop);
    if (group.lanes[1]) draw(group.lanes[1], group.plan.decoder);

    group.plan.done.forEach(function (moment, index) {
      var thumb = document.createElement("div");
      thumb.className = "thumb";
      thumb.style.setProperty("--i", index);
      thumb.style.setProperty("--t", moment / AXIS);
      group.strip.appendChild(thumb);
    });

    var ratio = (group.plan.total / (FRAMES * DECODE)).toFixed(2);
    group.time.innerHTML = "<b>" + ratio +
      "×</b><small>of the decode time</small>";
  });

  function draw(lane, blocks) {
    lane.textContent = "";
    blocks.forEach(function (block) {
      var el = document.createElement("div");
      el.className = "blk " + block.kind;
      el.style.setProperty("--x", block.x / AXIS);
      el.style.setProperty("--w", block.w / AXIS);
      lane.appendChild(el);
    });
  }

  var raf = null;

  function paint(units) {
    trace.style.setProperty("--p", Math.min(units, AXIS) / AXIS);
    groups.forEach(function (group) {
      group.root.classList.toggle("finished", units >= group.plan.total);
    });
  }

  function playTrace() {
    cancelAnimationFrame(raf);
    if (reduced) { paint(AXIS); return; }

    var slowest = Math.max(groups[0].plan.total, groups[1].plan.total);
    var start = null;
    function step(now) {
      if (start === null) start = now;
      var units = (now - start) / MS_PER_UNIT;
      paint(units);
      if (units < slowest) raf = requestAnimationFrame(step);
      else paint(AXIS);
    }
    paint(0);
    raf = requestAnimationFrame(step);
  }

  document.getElementById("replay").addEventListener("click", playTrace);
  playTrace();

  /* ---------- random access ---------- */

  var TOTAL_FRAMES = 14316;
  var GOP = 90;
  var WANTED = [934, 4522, 11711];
  var PAD = 6;                         // frames drawn either side of the window

  var film = document.getElementById("film");
  var scan = document.getElementById("film-scan");
  var runsHost = document.getElementById("film-runs");
  var marksHost = document.getElementById("film-marks");
  var catchHost = document.getElementById("catch");
  var zoomTitle = document.getElementById("zoom-title");
  var zoomCells = document.getElementById("zoom-cells");
  var zoomJump = document.getElementById("zoom-jump");
  var countDecoded = document.getElementById("count-decoded");
  var countSkipped = document.getElementById("count-skipped");
  var steps = document.querySelectorAll("#seek-steps li");

  var shots = WANTED.map(function (frame, index) {
    var key = Math.floor(frame / GOP) * GOP;
    return {
      index: index,
      frame: frame,
      key: key,
      decoded: frame - key + 1,
      from: key - PAD,
      to: frame + PAD
    };
  });

  var decodedTotal = shots.reduce(function (sum, shot) { return sum + shot.decoded; }, 0);

  var runEls = [], markEls = [], caughtEls = [];

  shots.forEach(function (shot) {
    var run = document.createElement("div");
    run.className = "run";
    run.style.setProperty("--x", shot.key / TOTAL_FRAMES);
    run.style.setProperty("--w", shot.decoded / TOTAL_FRAMES);
    runsHost.appendChild(run);
    runEls.push(run);

    var mark = document.createElement("div");
    mark.className = "mark";
    mark.style.setProperty("--x", shot.frame / TOTAL_FRAMES);
    mark.dataset.label = "Frame " + number(shot.frame);
    marksHost.appendChild(mark);
    markEls.push(mark);

    var caught = document.createElement("figure");
    caught.className = "caught";
    caught.innerHTML = '<i style="--i:' + shot.index + '"></i><span>Frame ' +
      number(shot.frame) + "<br>" + shot.decoded + " frames decoded from key frame " +
      number(shot.key) + "</span>";
    catchHost.appendChild(caught);
    caughtEls.push(caught);
  });

  steps[3].lastChild.textContent = " " + decodedTotal + " frames decoded, " +
    number(TOTAL_FRAMES - decodedTotal) + " never read at all.";

  var seekTimers = [];
  var decoded = 0;

  function counts(value) {
    decoded = value;
    countDecoded.textContent = number(decoded);
    countSkipped.textContent = number(TOTAL_FRAMES - decoded);
  }

  function later(ms, fn) { seekTimers.push(setTimeout(fn, ms)); }

  function drawWindow(shot) {
    // Draws the frames around one request, from a few before the key frame to
    // a few past the one that was asked for.
    zoomCells.textContent = "";
    var cells = [];
    for (var frame = shot.from; frame <= shot.to; frame++) {
      var cell = document.createElement("div");
      cell.className = "zcell";
      if (frame === shot.key) cell.classList.add("kf");
      zoomCells.appendChild(cell);
      cells.push(cell);
    }
    var count = cells.length;
    var keyAt = (PAD + 0.5) / count;
    var frameAt = (PAD + shot.decoded - 0.5) / count;
    zoomJump.style.left = (keyAt * 100) + "%";
    zoomJump.style.width = ((frameAt - keyAt) * 100) + "%";
    zoomJump.firstChild.textContent = "Seek back " + (shot.decoded - 1) + " frames";
    return cells;
  }

  function seekReset() {
    seekTimers.forEach(clearTimeout);
    seekTimers = [];
    film.className = "film";
    scan.style.transition = "none";
    scan.style.left = "0";
    zoomJump.classList.remove("on");
    zoomCells.textContent = "";
    zoomTitle.innerHTML = "Nothing decoded yet.";
    counts(0);
    runEls.concat(markEls, caughtEls).forEach(function (el) { el.classList.remove("on", "active"); });
    steps.forEach(function (step) { step.classList.remove("on"); });
  }

  function finishNow() {
    var last = shots[shots.length - 1];
    var cells = drawWindow(last);
    cells.forEach(function (cell, i) {
      if (i > PAD && i < PAD + last.decoded - 1) cell.classList.add("thrown");
      if (i === PAD + last.decoded - 1) cell.classList.add("kept");
    });
    zoomJump.classList.add("on");
    zoomTitle.innerHTML = "Frame <b>" + number(last.frame) +
      "</b> decoded from key frame <b>" + number(last.key) + "</b>.";
    film.classList.add("indexed");
    counts(decodedTotal);
    runEls.concat(markEls, caughtEls).forEach(function (el) { el.classList.add("on"); });
    steps.forEach(function (step) { step.classList.add("on"); });
  }

  function seekPlay() {
    seekReset();
    if (reduced) { finishNow(); return; }

    var INDEX_MS = 2600;
    var SLOT = 4000;

    steps[0].classList.add("on");
    film.classList.add("indexing");
    zoomTitle.innerHTML = "Reading the index: every packet is demuxed, none is decoded.";
    requestAnimationFrame(function () {
      scan.style.transition = "left " + INDEX_MS + "ms linear";
      scan.style.left = "100%";
    });

    later(INDEX_MS + 60, function () {
      film.classList.remove("indexing");
      film.classList.add("indexed");
      steps[1].classList.add("on");
    });

    shots.forEach(function (shot, order) {
      var base = INDEX_MS + 60 + order * SLOT;

      later(base, function () {
        var cells = drawWindow(shot);
        markEls[shot.index].classList.add("on", "active");
        zoomTitle.innerHTML = "Frame <b>" + number(shot.frame) +
          "</b> needs key frame <b>" + number(shot.key) + "</b> first.";

        later(350, function () { zoomJump.classList.add("on"); });

        later(1700, function () {
          steps[2].classList.add("on");
          zoomTitle.innerHTML = "Decoding forward from <b>" + number(shot.key) +
            "</b>, keeping only <b>" + number(shot.frame) + "</b>.";
          runEls[shot.index].classList.add("on");

          var perCell = 1900 / shot.decoded;
          for (var i = 0; i < shot.decoded; i++) {
            (function (i) {
              later(i * perCell, function () {
                var cell = cells[PAD + i];
                cell.classList.add(i === shot.decoded - 1 ? "kept" : "thrown");
                counts(decoded + 1);
              });
            })(i);
          }
        });

        later(3700, function () {
          markEls[shot.index].classList.remove("active");
          caughtEls[shot.index].classList.add("on");
        });
      });
    });

    later(INDEX_MS + 60 + shots.length * SLOT, function () {
      steps[3].classList.add("on");
      zoomTitle.innerHTML = "Three frames out of " + number(TOTAL_FRAMES) +
        ", for the cost of " + decodedTotal + ".";
    });
  }

  once(document.getElementById("seekdemo"), seekPlay);
  document.getElementById("seek-replay").addEventListener("click", seekPlay);

  /* ---------- helpers ---------- */

  function number(value) { return value.toLocaleString("en-US"); }

  function once(target, fn) {
    if (!("IntersectionObserver" in window)) { fn(); return; }
    var observer = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        if (entry.isIntersecting) { observer.disconnect(); fn(); }
      });
    }, { threshold: .25 });
    observer.observe(target);
  }
})();
