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

  // The tolerance of the approximate mode: a frame is read as the key frame
  // nearest to it when that one is no further away than this, else exactly.
  var APPROXIMATE = 45;

  // One entry per frame asked for, in each mode. `key` is where decoding
  // starts, `decoded` how many frames it costs, and `kept` the frame handed
  // over, which is the key frame itself when the number snapped to it.
  function seekPlan(approximate) {
    return WANTED.map(function (frame, index) {
      var before = Math.floor(frame / GOP) * GOP;
      var after = before + GOP;
      var nearest = frame - before <= after - frame ? before : after;
      var snapped = approximate && Math.abs(nearest - frame) <= APPROXIMATE;
      var key = snapped ? nearest : before;
      var kept = snapped ? nearest : frame;
      return {
        index: index,
        frame: frame,
        key: key,
        kept: kept,
        snapped: snapped,
        moved: Math.abs(nearest - frame),
        decoded: snapped ? 1 : frame - before + 1,
        from: Math.min(key, frame) - PAD,
        to: Math.max(key, frame) + PAD
      };
    });
  }

  var PLANS = { exact: seekPlan(false), approximate: seekPlan(true) };

  function cost(shots) {
    return shots.reduce(function (sum, shot) { return sum + shot.decoded; }, 0);
  }

  var modeButtons = {
    exact: document.getElementById("mode-exact"),
    approximate: document.getElementById("mode-approximate")
  };
  var askedKey = document.getElementById("key-asked");
  var seekCall = document.getElementById("seek-call");
  var seekCost = document.getElementById("seek-cost");

  modeButtons.approximate.textContent = "approximate=" + APPROXIMATE;

  var mode = "exact";
  var shots = PLANS[mode];
  var decodedTotal = cost(shots);
  var runEls = [], markEls = [], caughtEls = [];

  function drawSeek() {
    shots = PLANS[mode];
    decodedTotal = cost(shots);
    runsHost.textContent = "";
    marksHost.textContent = "";
    catchHost.textContent = "";
    runEls = [];
    markEls = [];
    caughtEls = [];

    shots.forEach(function (shot) {
      var run = document.createElement("div");
      run.className = "run";
      run.style.setProperty("--x", shot.key / TOTAL_FRAMES);
      // A single decoded frame would be invisible on a bar this wide.
      run.style.setProperty("--w", Math.max(shot.decoded, GOP / 3) / TOTAL_FRAMES);
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
      caught.innerHTML = '<i style="--i:' + shot.index + '"></i><span>' +
        (shot.snapped
          ? "Key frame " + number(shot.key) + "<br>" + shot.moved +
            " frames from the " + number(shot.frame) + " asked for"
          : "Frame " + number(shot.frame) + "<br>" + shot.decoded +
            " frames decoded from key frame " + number(shot.key)) +
        "</span>";
      catchHost.appendChild(caught);
      caughtEls.push(caught);
    });

    askedKey.hidden = mode === "exact";
    seekCall.innerHTML = 'iterframes.<span class="f">read</span>(' +
      '<span class="s">"bunny.mp4"</span>, frames=[<span class="n">934</span>, ' +
      '<span class="n">4522</span>, <span class="n">11711</span>]' +
      (mode === "exact" ? "" : ', approximate=<span class="n">' + APPROXIMATE + "</span>") + ")";
    seekCost.textContent = mode === "exact"
      ? decodedTotal + " frames decoded for " + shots.length + " frames asked for."
      : decodedTotal + " frames decoded instead of " + cost(PLANS.exact) +
        ", each at most " + shots.reduce(function (most, shot) {
          return Math.max(most, shot.moved);
        }, 0) + " frames from the one asked for.";

    stepsFor(mode).forEach(function (text, index) {
      steps[index].innerHTML = '<span class="dot"></span><b>' + text[0] + "</b> " + text[1];
    });
  }

  function stepsFor(mode) {
    var indexed = ["Index the file once.", "Every packet is demuxed and none is " +
      "decoded, which is enough to learn where each frame sits and which frames " +
      "are key frames."];
    var first = shots[0];
    if (mode === "exact") {
      return [
        indexed,
        ["Jump back to a key frame.", "Frame " + number(first.frame) + " cannot be " +
          "decoded on its own, so the decoder seeks to frame " + number(first.key) +
          ", the key frame in front of it."],
        ["Decode forward and drop.", "The frames in between are decoded so the next " +
          "one can be, then thrown away without ever being converted to RGB."],
        ["Hand over " + shots.length + " frames.", decodedTotal + " frames decoded, " +
          number(TOTAL_FRAMES - decodedTotal) + " never read at all."]
      ];
    }
    return [
      indexed,
      ["Snap to the nearest key frame.", "Frame " + number(first.frame) + " is " +
        first.moved + " frames from key frame " + number(first.key) + ", inside the " +
        APPROXIMATE + " frames allowed, so that key frame stands in for it."],
      ["Decode one frame.", "A key frame decodes on its own, so nothing in between " +
        "is decoded and nothing is thrown away."],
      ["Hand over " + shots.length + " frames.", decodedTotal + " frames decoded, " +
        number(TOTAL_FRAMES - decodedTotal) + " never read at all."]
    ];
  }

  var seekTimers = [];
  var decoded = 0;

  function counts(value) {
    decoded = value;
    countDecoded.textContent = number(decoded);
    countSkipped.textContent = number(TOTAL_FRAMES - decoded);
  }

  function later(ms, fn) { seekTimers.push(setTimeout(fn, ms)); }

  function drawWindow(shot) {
    // Draws the frames around one request, from a few before the first of the
    // key frame and the frame asked for to a few past the last of the two.
    zoomCells.textContent = "";
    var cells = [];
    for (var frame = shot.from; frame <= shot.to; frame++) {
      var cell = document.createElement("div");
      cell.className = "zcell";
      if (frame === shot.key) cell.classList.add("kf");
      if (shot.snapped && frame === shot.frame) cell.classList.add("asked");
      zoomCells.appendChild(cell);
      cells.push(cell);
    }
    var count = cells.length;
    var keyAt = (shot.key - shot.from + 0.5) / count;
    var frameAt = (shot.frame - shot.from + 0.5) / count;
    zoomJump.style.left = (Math.min(keyAt, frameAt) * 100) + "%";
    zoomJump.style.width = (Math.abs(frameAt - keyAt) * 100) + "%";
    zoomJump.firstChild.textContent = shot.snapped
      ? "Read " + shot.moved + " frames earlier"
      : "Seek back " + (shot.decoded - 1) + " frames";
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
    var keyAt = last.key - last.from;
    cells.forEach(function (cell, i) {
      if (i > keyAt && i < keyAt + last.decoded - 1) cell.classList.add("thrown");
      if (i === keyAt + last.decoded - 1) cell.classList.add("kept");
    });
    zoomJump.classList.add("on");
    zoomTitle.innerHTML = last.snapped
      ? "Key frame <b>" + number(last.key) + "</b> read for frame <b>" +
        number(last.frame) + "</b>."
      : "Frame <b>" + number(last.frame) + "</b> decoded from key frame <b>" +
        number(last.key) + "</b>.";
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
        var keyAt = shot.key - shot.from;
        markEls[shot.index].classList.add("on", "active");
        zoomTitle.innerHTML = shot.snapped
          ? "Frame <b>" + number(shot.frame) + "</b> is " + shot.moved +
            " frames from key frame <b>" + number(shot.key) + "</b>."
          : "Frame <b>" + number(shot.frame) + "</b> needs key frame <b>" +
            number(shot.key) + "</b> first.";

        later(350, function () { zoomJump.classList.add("on"); });

        later(1700, function () {
          steps[2].classList.add("on");
          zoomTitle.innerHTML = shot.snapped
            ? "Decoding key frame <b>" + number(shot.key) + "</b> alone."
            : "Decoding forward from <b>" + number(shot.key) +
              "</b>, keeping only <b>" + number(shot.frame) + "</b>.";
          runEls[shot.index].classList.add("on");

          var perCell = 1900 / shot.decoded;
          for (var i = 0; i < shot.decoded; i++) {
            (function (i) {
              later(i * perCell, function () {
                var cell = cells[keyAt + i];
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
      zoomTitle.innerHTML = shots.length + " frames out of " + number(TOTAL_FRAMES) +
        ", for the cost of " + decodedTotal + ".";
    });
  }

  Object.keys(modeButtons).forEach(function (name) {
    modeButtons[name].addEventListener("click", function () {
      if (mode === name) return;
      mode = name;
      Object.keys(modeButtons).forEach(function (other) {
        modeButtons[other].classList.toggle("on", other === mode);
        modeButtons[other].setAttribute("aria-pressed", other === mode ? "true" : "false");
      });
      drawSeek();
      seekPlay();
    });
  });

  drawSeek();
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
