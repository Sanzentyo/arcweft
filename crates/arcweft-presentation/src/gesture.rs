use crate::input::{InteractionTarget, PointerId, ViewportPoint};

/// Gesture contender registered for one pointer sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GestureKind {
    Tap,
    Drag,
    ScrollX,
    ScrollY,
}

/// Deterministic threshold policy for the gesture arena.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GestureArenaConfig {
    pub tap_slop_px: f32,
    pub drag_threshold_px: f32,
    pub scroll_threshold_px: f32,
}

/// One active pointer sequence inside the gesture arena.
#[derive(Clone, Debug, PartialEq)]
pub struct GestureSession {
    pointer: PointerId,
    target: InteractionTarget,
    start: ViewportPoint,
    latest: ViewportPoint,
    contenders: Vec<GestureKind>,
    winner: Option<GestureKind>,
}

/// Output from a gesture arena update.
#[derive(Clone, Debug, PartialEq)]
pub enum GestureOutcome {
    Pending,
    Won {
        pointer: PointerId,
        target: InteractionTarget,
        winner: GestureKind,
    },
    Completed {
        pointer: PointerId,
        target: InteractionTarget,
        winner: Option<GestureKind>,
    },
    Cancelled {
        pointer: PointerId,
        target: InteractionTarget,
        winner: Option<GestureKind>,
    },
    MissingPointer {
        pointer: PointerId,
    },
}

/// Sans I/O gesture arbitration state.
#[derive(Clone, Debug, PartialEq)]
pub struct GestureArena {
    config: GestureArenaConfig,
    sessions: Vec<GestureSession>,
}

impl GestureArenaConfig {
    pub const fn new(tap_slop_px: f32, drag_threshold_px: f32, scroll_threshold_px: f32) -> Self {
        Self {
            tap_slop_px,
            drag_threshold_px,
            scroll_threshold_px,
        }
    }
}

impl Default for GestureArenaConfig {
    fn default() -> Self {
        Self::new(6.0, 8.0, 8.0)
    }
}

impl GestureSession {
    pub fn new(
        pointer: PointerId,
        target: InteractionTarget,
        start: ViewportPoint,
        contenders: Vec<GestureKind>,
    ) -> Self {
        Self {
            pointer,
            target,
            start,
            latest: start,
            contenders,
            winner: None,
        }
    }

    pub const fn pointer(&self) -> PointerId {
        self.pointer
    }

    pub const fn target(&self) -> &InteractionTarget {
        &self.target
    }

    pub const fn winner(&self) -> Option<GestureKind> {
        self.winner
    }

    pub fn contenders(&self) -> &[GestureKind] {
        &self.contenders
    }

    fn has_contender(&self, kind: GestureKind) -> bool {
        self.contenders.contains(&kind)
    }

    fn movement_to(&self, position: ViewportPoint) -> PointerMovement {
        PointerMovement {
            dx: position.x - self.start.x,
            dy: position.y - self.start.y,
        }
    }
}

impl GestureArena {
    pub fn new(config: GestureArenaConfig) -> Self {
        Self {
            config,
            sessions: Vec::new(),
        }
    }

    pub const fn config(&self) -> GestureArenaConfig {
        self.config
    }

    pub fn sessions(&self) -> &[GestureSession] {
        &self.sessions
    }

    pub fn begin(
        &mut self,
        pointer: PointerId,
        target: InteractionTarget,
        start: ViewportPoint,
        contenders: Vec<GestureKind>,
    ) {
        self.remove(pointer);
        self.sessions
            .push(GestureSession::new(pointer, target, start, contenders));
    }

    pub fn update(&mut self, pointer: PointerId, position: ViewportPoint) -> GestureOutcome {
        let config = self.config;
        let Some(session) = self.session_mut(pointer) else {
            return GestureOutcome::MissingPointer { pointer };
        };
        session.latest = position;
        if let Some(winner) = session.winner {
            return GestureOutcome::Won {
                pointer,
                target: session.target.clone(),
                winner,
            };
        }

        let movement = session.movement_to(position);
        if let Some(winner) = choose_winner(session, movement, config) {
            session.winner = Some(winner);
            return GestureOutcome::Won {
                pointer,
                target: session.target.clone(),
                winner,
            };
        }
        GestureOutcome::Pending
    }

    pub fn end(&mut self, pointer: PointerId, position: ViewportPoint) -> GestureOutcome {
        let Some(mut session) = self.remove(pointer) else {
            return GestureOutcome::MissingPointer { pointer };
        };
        session.latest = position;
        if session.winner.is_none()
            && session.has_contender(GestureKind::Tap)
            && session.movement_to(position).distance_squared()
                <= self.config.tap_slop_px * self.config.tap_slop_px
        {
            session.winner = Some(GestureKind::Tap);
        }
        GestureOutcome::Completed {
            pointer,
            target: session.target,
            winner: session.winner,
        }
    }

    pub fn cancel(&mut self, pointer: PointerId) -> GestureOutcome {
        let Some(session) = self.remove(pointer) else {
            return GestureOutcome::MissingPointer { pointer };
        };
        GestureOutcome::Cancelled {
            pointer,
            target: session.target,
            winner: session.winner,
        }
    }

    fn session_mut(&mut self, pointer: PointerId) -> Option<&mut GestureSession> {
        self.sessions
            .iter_mut()
            .find(|session| session.pointer() == pointer)
    }

    fn remove(&mut self, pointer: PointerId) -> Option<GestureSession> {
        self.sessions
            .iter()
            .position(|session| session.pointer() == pointer)
            .map(|index| self.sessions.remove(index))
    }
}

impl Default for GestureArena {
    fn default() -> Self {
        Self::new(GestureArenaConfig::default())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PointerMovement {
    dx: f32,
    dy: f32,
}

impl PointerMovement {
    fn abs_x(self) -> f32 {
        self.dx.abs()
    }

    fn abs_y(self) -> f32 {
        self.dy.abs()
    }

    fn distance_squared(self) -> f32 {
        (self.dx * self.dx) + (self.dy * self.dy)
    }
}

fn choose_winner(
    session: &GestureSession,
    movement: PointerMovement,
    config: GestureArenaConfig,
) -> Option<GestureKind> {
    if movement.abs_y() >= config.scroll_threshold_px
        && movement.abs_y() >= movement.abs_x()
        && session.has_contender(GestureKind::ScrollY)
    {
        return Some(GestureKind::ScrollY);
    }
    if movement.abs_x() >= config.scroll_threshold_px
        && movement.abs_x() > movement.abs_y()
        && session.has_contender(GestureKind::ScrollX)
    {
        return Some(GestureKind::ScrollX);
    }
    if movement.distance_squared() >= config.drag_threshold_px * config.drag_threshold_px
        && session.has_contender(GestureKind::Drag)
    {
        return Some(GestureKind::Drag);
    }
    None
}
