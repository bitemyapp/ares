use crate::assert_acyclic;
use crate::assert_no_forwarding_pointers;
use crate::assert_no_junior_pointers;
use crate::hamt::Hamt;
use crate::jets::cold;
use crate::jets::cold::Cold;
use crate::jets::hot::Hot;
use crate::jets::warm::Warm;
use crate::jets::JetErr;
use crate::mem::unifying_equality;
use crate::mem::NockStack;
use crate::newt::Newt;
use crate::noun;
use crate::noun::{Atom, Cell, IndirectAtom, Noun, Slots, D, T};
use crate::serf::TERMINATOR;
use ares_macros::tas;
use assert_no_alloc::assert_no_alloc;
use bitvec::prelude::{BitSlice, Lsb0};
use either::Either::*;
use std::sync::atomic::Ordering;
use std::sync::Arc;

crate::gdb!();

#[derive(Copy, Clone)]
#[repr(u8)]
enum TodoCons {
    ComputeHead,
    ComputeTail,
    Cons,
}

#[derive(Copy, Clone)]
struct NockCons {
    todo: TodoCons,
    head: Noun,
    tail: Noun,
}

#[derive(Copy, Clone)]
struct Nock0 {
    axis: Atom,
}

#[derive(Copy, Clone)]
struct Nock1 {
    noun: Noun,
}

#[derive(Copy, Clone)]
#[repr(u8)]
enum Todo2 {
    ComputeSubject,
    ComputeFormula,
    ComputeResult,
    RestoreSubject,
}

#[derive(Copy, Clone)]
struct Nock2 {
    todo: Todo2,
    subject: Noun,
    formula: Noun,
    tail: bool,
}

#[derive(Copy, Clone)]
#[repr(u8)]
enum Todo3 {
    ComputeChild,
    ComputeType,
}

#[derive(Copy, Clone)]
struct Nock3 {
    todo: Todo3,
    child: Noun,
}

#[derive(Copy, Clone)]
#[repr(u8)]
enum Todo4 {
    ComputeChild,
    Increment,
}

#[derive(Copy, Clone)]
struct Nock4 {
    todo: Todo4,
    child: Noun,
}

#[derive(Copy, Clone)]
#[repr(u8)]
enum Todo5 {
    ComputeLeftChild,
    ComputeRightChild,
    TestEquals,
}

#[derive(Copy, Clone)]
struct Nock5 {
    todo: Todo5,
    left: Noun,
    right: Noun,
}

#[derive(Copy, Clone)]
#[repr(u8)]
enum Todo6 {
    ComputeTest,
    ComputeBranch,
}

#[derive(Copy, Clone)]
struct Nock6 {
    todo: Todo6,
    test: Noun,
    zero: Noun,
    once: Noun,
    tail: bool,
}

#[derive(Copy, Clone)]
#[repr(u8)]
enum Todo7 {
    ComputeSubject,
    ComputeResult,
    RestoreSubject,
}

#[derive(Copy, Clone)]
struct Nock7 {
    todo: Todo7,
    subject: Noun,
    formula: Noun,
    tail: bool,
}

#[derive(Copy, Clone)]
#[repr(u8)]
enum Todo8 {
    ComputeSubject,
    ComputeResult,
    RestoreSubject,
}

#[derive(Copy, Clone)]
struct Nock8 {
    todo: Todo8,
    pin: Noun,
    formula: Noun,
    tail: bool,
}

#[derive(Copy, Clone)]
#[repr(u8)]
enum Todo9 {
    ComputeCore,
    ComputeResult,
    RestoreSubject,
}

#[derive(Copy, Clone)]
struct Nock9 {
    todo: Todo9,
    axis: Atom,
    core: Noun,
    tail: bool,
}

#[derive(Copy, Clone)]
#[repr(u8)]
enum Todo10 {
    ComputeTree,
    ComputePatch,
    Edit,
}

#[derive(Copy, Clone)]
struct Nock10 {
    todo: Todo10,
    axis: Atom,
    tree: Noun,
    patch: Noun,
}

#[derive(Copy, Clone)]
#[repr(u8)]
enum Todo11D {
    ComputeHint,
    ComputeResult,
    Done,
}

#[derive(Copy, Clone)]
struct Nock11D {
    todo: Todo11D,
    tag: Atom,
    hint: Noun,
    body: Noun,
    tail: bool,
}

#[derive(Copy, Clone)]
enum Todo11S {
    ComputeResult,
    Done,
}

#[derive(Copy, Clone)]
struct Nock11S {
    todo: Todo11S,
    tag: Atom,
    body: Noun,
    tail: bool,
}

#[derive(Copy, Clone)]
enum Todo12 {
    ComputeReff,
    ComputePath,
    Scry,
}

#[derive(Copy, Clone)]
struct Nock12 {
    todo: Todo12,
    reff: Noun,
    path: Noun,
}

#[derive(Copy, Clone)]
enum NockWork {
    Done,
    Ret,
    WorkCons(NockCons),
    Work0(Nock0),
    Work1(Nock1),
    Work2(Nock2),
    Work3(Nock3),
    Work4(Nock4),
    Work5(Nock5),
    Work6(Nock6),
    Work7(Nock7),
    Work8(Nock8),
    Work9(Nock9),
    Work10(Nock10),
    Work11D(Nock11D),
    Work11S(Nock11S),
    Work12(Nock12),
}

pub struct Context {
    pub stack: NockStack,
    // For printing slogs; if None, print to stdout; Option slated to be removed
    pub newt: Newt,
    pub cold: Cold,
    pub warm: Warm,
    pub hot: Hot,
    //  XX: persistent memo cache
    // Per-event cache; option to share cache with virtualized events
    pub cache: Hamt<Noun>,
    pub scry_stack: Noun,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Error {
    ScryBlocked(Noun),      // path
    ScryCrashed(Noun),      // trace
    Deterministic(Noun),    // trace
    NonDeterministic(Noun), // trace
}

impl From<noun::Error> for Error {
    fn from(_: noun::Error) -> Self {
        Error::Deterministic(D(0))
    }
}

impl From<cold::Error> for Error {
    fn from(_: cold::Error) -> Self {
        Error::Deterministic(D(0))
    }
}

#[allow(unused_variables)]
fn debug_assertions(stack: &mut NockStack, noun: Noun) {
    assert_acyclic!(noun);
    assert_no_forwarding_pointers!(noun);
    assert_no_junior_pointers!(stack, noun);
}

/** Interpret nock */
pub fn interpret(context: &mut Context, mut subject: Noun, formula: Noun) -> Result<Noun, Error> {
    let terminator = Arc::clone(&TERMINATOR);
    let orig_subject = subject; // for debugging
    let virtual_frame: *const u64 = context.stack.get_frame_pointer();
    let mut res: Noun = D(0);

    // Setup stack for Nock computation
    unsafe {
        context.stack.frame_push(1);
        // Bottom of mean stack
        *(context.stack.local_noun_pointer(0)) = D(0);
        *(context.stack.push()) = NockWork::Done;
    };

    // DO NOT REMOVE THIS ASSERTION
    //
    // If you need to allocate for debugging, wrap the debugging code in
    //
    // ```
    // permit_alloc(|| {
    //   your.code.goes.here()
    // })
    // ```
    //
    // (See https://docs.rs/assert_no_alloc/latest/assert_no_alloc/#advanced-use)
    let nock = assert_no_alloc(|| unsafe {
        push_formula(&mut context.stack, formula, true)?;

        loop {
            let work: NockWork = *context.stack.top();
            match work {
                NockWork::Done => {
                    let stack = &mut context.stack;

                    debug_assertions(stack, orig_subject);
                    debug_assertions(stack, subject);
                    debug_assertions(stack, res);

                    stack.preserve(&mut context.cache);
                    stack.preserve(&mut context.cold);
                    stack.preserve(&mut context.warm);
                    stack.preserve(&mut res);
                    stack.frame_pop();

                    debug_assertions(stack, orig_subject);
                    debug_assertions(stack, res);

                    break Ok(res);
                }
                NockWork::Ret => {
                    let stack = &mut context.stack;

                    debug_assertions(stack, orig_subject);
                    debug_assertions(stack, subject);
                    debug_assertions(stack, res);

                    stack.preserve(&mut context.cache);
                    stack.preserve(&mut context.cold);
                    stack.preserve(&mut context.warm);
                    stack.preserve(&mut res);
                    stack.frame_pop();

                    debug_assertions(stack, orig_subject);
                    debug_assertions(stack, res);
                }
                NockWork::WorkCons(mut cons) => match cons.todo {
                    TodoCons::ComputeHead => {
                        cons.todo = TodoCons::ComputeTail;
                        *context.stack.top() = NockWork::WorkCons(cons);
                        push_formula(&mut context.stack, cons.head, false)?;
                    }
                    TodoCons::ComputeTail => {
                        cons.todo = TodoCons::Cons;
                        cons.head = res;
                        *context.stack.top() = NockWork::WorkCons(cons);
                        push_formula(&mut context.stack, cons.tail, false)?;
                    }
                    TodoCons::Cons => {
                        let stack = &mut context.stack;
                        res = T(stack, &[cons.head, res]);
                        stack.pop::<NockWork>();
                    }
                },
                NockWork::Work0(zero) => {
                    if let Ok(noun) = subject.slot_atom(zero.axis) {
                        res = noun;
                        context.stack.pop::<NockWork>();
                    } else {
                        // Axis invalid for input Noun
                        break Err(Error::Deterministic(D(0)));
                    }
                }
                NockWork::Work1(once) => {
                    res = once.noun;
                    context.stack.pop::<NockWork>();
                }
                NockWork::Work2(mut vale) => {
                    if (*terminator).load(Ordering::Relaxed) {
                        break Err(Error::NonDeterministic(D(0)));
                    }

                    match vale.todo {
                        Todo2::ComputeSubject => {
                            vale.todo = Todo2::ComputeFormula;
                            *context.stack.top() = NockWork::Work2(vale);
                            push_formula(&mut context.stack, vale.subject, false)?;
                        }
                        Todo2::ComputeFormula => {
                            vale.todo = Todo2::ComputeResult;
                            vale.subject = res;
                            *context.stack.top() = NockWork::Work2(vale);
                            push_formula(&mut context.stack, vale.formula, false)?;
                        }
                        Todo2::ComputeResult => {
                            if !cfg!(feature = "sham_hints") {
                                if let Some(jet) = context.warm.find_jet(
                                    &mut context.stack,
                                    &mut vale.subject,
                                    &mut res,
                                ) {
                                    match jet(context, vale.subject) {
                                        Ok(jet_res) => {
                                            res = jet_res;
                                            context.stack.pop::<NockWork>();
                                            continue;
                                        }
                                        Err(JetErr::Punt) => {}
                                        Err(err) => {
                                            break Err(err.into());
                                        }
                                    }
                                }
                            };

                            let stack = &mut context.stack;
                            if vale.tail {
                                stack.pop::<NockWork>();
                                subject = vale.subject;
                                push_formula(stack, res, true)?;
                            } else {
                                vale.todo = Todo2::RestoreSubject;
                                std::mem::swap(&mut vale.subject, &mut subject);
                                *stack.top() = NockWork::Work2(vale);

                                debug_assertions(stack, orig_subject);
                                debug_assertions(stack, subject);
                                debug_assertions(stack, res);

                                mean_frame_push(stack, 0);
                                *stack.push() = NockWork::Ret;
                                push_formula(stack, res, true)?;
                            }
                        }
                        Todo2::RestoreSubject => {
                            let stack = &mut context.stack;

                            subject = vale.subject;
                            stack.pop::<NockWork>();

                            debug_assertions(stack, orig_subject);
                            debug_assertions(stack, subject);
                            debug_assertions(stack, res);
                        }
                    }
                }
                NockWork::Work3(mut thee) => match thee.todo {
                    Todo3::ComputeChild => {
                        thee.todo = Todo3::ComputeType;
                        *context.stack.top() = NockWork::Work3(thee);
                        push_formula(&mut context.stack, thee.child, false)?;
                    }
                    Todo3::ComputeType => {
                        res = if res.is_cell() { D(0) } else { D(1) };
                        context.stack.pop::<NockWork>();
                    }
                },
                NockWork::Work4(mut four) => match four.todo {
                    Todo4::ComputeChild => {
                        four.todo = Todo4::Increment;
                        *context.stack.top() = NockWork::Work4(four);
                        push_formula(&mut context.stack, four.child, false)?;
                    }
                    Todo4::Increment => {
                        if let Ok(atom) = res.as_atom() {
                            res = inc(&mut context.stack, atom).as_noun();
                            context.stack.pop::<NockWork>();
                        } else {
                            // Cannot increment (Nock 4) a cell
                            break Err(Error::Deterministic(D(0)));
                        }
                    }
                },
                NockWork::Work5(mut five) => match five.todo {
                    Todo5::ComputeLeftChild => {
                        five.todo = Todo5::ComputeRightChild;
                        *context.stack.top() = NockWork::Work5(five);
                        push_formula(&mut context.stack, five.left, false)?;
                    }
                    Todo5::ComputeRightChild => {
                        five.todo = Todo5::TestEquals;
                        five.left = res;
                        *context.stack.top() = NockWork::Work5(five);
                        push_formula(&mut context.stack, five.right, false)?;
                    }
                    Todo5::TestEquals => {
                        let stack = &mut context.stack;
                        let saved_value_ptr = &mut five.left;
                        res = if unifying_equality(stack, &mut res, saved_value_ptr) {
                            D(0)
                        } else {
                            D(1)
                        };
                        stack.pop::<NockWork>();
                    }
                },
                NockWork::Work6(mut cond) => match cond.todo {
                    Todo6::ComputeTest => {
                        cond.todo = Todo6::ComputeBranch;
                        *context.stack.top() = NockWork::Work6(cond);
                        push_formula(&mut context.stack, cond.test, false)?;
                    }
                    Todo6::ComputeBranch => {
                        let stack = &mut context.stack;
                        stack.pop::<NockWork>();
                        if let Left(direct) = res.as_either_direct_allocated() {
                            if direct.data() == 0 {
                                push_formula(stack, cond.zero, cond.tail)?;
                            } else if direct.data() == 1 {
                                push_formula(stack, cond.once, cond.tail)?;
                            } else {
                                // Test branch of Nock 6 must return 0 or 1
                                break Err(Error::Deterministic(D(0)));
                            }
                        } else {
                            // Test branch of Nock 6 must return a direct atom
                            break Err(Error::Deterministic(D(0)));
                        }
                    }
                },
                NockWork::Work7(mut pose) => match pose.todo {
                    Todo7::ComputeSubject => {
                        pose.todo = Todo7::ComputeResult;
                        *context.stack.top() = NockWork::Work7(pose);
                        push_formula(&mut context.stack, pose.subject, false)?;
                    }
                    Todo7::ComputeResult => {
                        let stack = &mut context.stack;
                        if pose.tail {
                            stack.pop::<NockWork>();
                            subject = res;
                            push_formula(stack, pose.formula, true)?;
                        } else {
                            pose.todo = Todo7::RestoreSubject;
                            pose.subject = subject;
                            *stack.top() = NockWork::Work7(pose);
                            subject = res;
                            push_formula(stack, pose.formula, false)?;
                        }
                    }
                    Todo7::RestoreSubject => {
                        subject = pose.subject;
                        context.stack.pop::<NockWork>();
                    }
                },
                NockWork::Work8(mut pins) => match pins.todo {
                    Todo8::ComputeSubject => {
                        pins.todo = Todo8::ComputeResult;
                        *context.stack.top() = NockWork::Work8(pins);
                        push_formula(&mut context.stack, pins.pin, false)?;
                    }
                    Todo8::ComputeResult => {
                        let stack = &mut context.stack;
                        if pins.tail {
                            subject = T(stack, &[res, subject]);
                            stack.pop::<NockWork>();
                            push_formula(stack, pins.formula, true)?;
                        } else {
                            pins.todo = Todo8::RestoreSubject;
                            pins.pin = subject;
                            *stack.top() = NockWork::Work8(pins);
                            subject = T(stack, &[res, subject]);
                            push_formula(stack, pins.formula, false)?;
                        }
                    }
                    Todo8::RestoreSubject => {
                        subject = pins.pin;
                        context.stack.pop::<NockWork>();
                    }
                },
                NockWork::Work9(mut kale) => {
                    if (*terminator).load(Ordering::Relaxed) {
                        break Err(Error::NonDeterministic(D(0)));
                    }

                    match kale.todo {
                        Todo9::ComputeCore => {
                            kale.todo = Todo9::ComputeResult;
                            *context.stack.top() = NockWork::Work9(kale);
                            push_formula(&mut context.stack, kale.core, false)?;
                        }
                        Todo9::ComputeResult => {
                            if let Ok(mut formula) = res.slot_atom(kale.axis) {
                                if !cfg!(feature = "sham_hints") {
                                    if let Some(jet) = context.warm.find_jet(
                                        &mut context.stack,
                                        &mut res,
                                        &mut formula,
                                    ) {
                                        match jet(context, res) {
                                            Ok(jet_res) => {
                                                res = jet_res;
                                                context.stack.pop::<NockWork>();
                                                continue;
                                            }
                                            Err(JetErr::Punt) => {}
                                            Err(err) => {
                                                break Err(err.into());
                                            }
                                        }
                                    }
                                };

                                let stack = &mut context.stack;
                                if kale.tail {
                                    stack.pop::<NockWork>();
                                    subject = res;
                                    push_formula(stack, formula, true)?;
                                } else {
                                    kale.todo = Todo9::RestoreSubject;
                                    kale.core = subject;
                                    *stack.top() = NockWork::Work9(kale);

                                    debug_assertions(stack, orig_subject);
                                    debug_assertions(stack, subject);
                                    debug_assertions(stack, res);

                                    subject = res;
                                    mean_frame_push(stack, 0);
                                    *stack.push() = NockWork::Ret;
                                    push_formula(stack, formula, true)?;
                                }
                            } else {
                                // Axis into core must be atom
                                break Err(Error::Deterministic(D(0)));
                            }
                        }
                        Todo9::RestoreSubject => {
                            let stack = &mut context.stack;

                            subject = kale.core;
                            stack.pop::<NockWork>();

                            debug_assertions(stack, orig_subject);
                            debug_assertions(stack, subject);
                            debug_assertions(stack, res);
                        }
                    }
                }
                NockWork::Work10(mut diet) => {
                    match diet.todo {
                        Todo10::ComputeTree => {
                            diet.todo = Todo10::ComputePatch; // should we compute patch then tree?
                            *context.stack.top() = NockWork::Work10(diet);
                            push_formula(&mut context.stack, diet.tree, false)?;
                        }
                        Todo10::ComputePatch => {
                            diet.todo = Todo10::Edit;
                            diet.tree = res;
                            *context.stack.top() = NockWork::Work10(diet);
                            push_formula(&mut context.stack, diet.patch, false)?;
                        }
                        Todo10::Edit => {
                            res = edit(&mut context.stack, diet.axis.as_bitslice(), res, diet.tree);
                            context.stack.pop::<NockWork>();
                        }
                    }
                }
                NockWork::Work11D(mut dint) => match dint.todo {
                    Todo11D::ComputeHint => {
                        match hint::match_pre_hint(context, subject, dint.tag, dint.hint, dint.body)
                        {
                            Ok(Some(found)) => {
                                res = found;
                                context.stack.pop::<NockWork>();
                            }
                            Ok(None) => {
                                dint.todo = Todo11D::ComputeResult;
                                *context.stack.top() = NockWork::Work11D(dint);
                                push_formula(&mut context.stack, dint.hint, false)?;
                            }
                            Err(err) => {
                                break Err(err);
                            }
                        }
                    }
                    Todo11D::ComputeResult => {
                        match hint::match_pre_nock(
                            context,
                            subject,
                            dint.tag,
                            Some((dint.hint, res)),
                            dint.body,
                        ) {
                            Ok(Some(found)) => {
                                res = found;
                                context.stack.pop::<NockWork>();
                            }
                            Ok(None) => {
                                if dint.tail {
                                    context.stack.pop::<NockWork>();
                                } else {
                                    dint.todo = Todo11D::Done;
                                    dint.hint = res;
                                    *context.stack.top() = NockWork::Work11D(dint);
                                }
                                push_formula(&mut context.stack, dint.body, dint.tail)?;
                            }
                            Err(err) => {
                                break Err(err);
                            }
                        }
                    }
                    Todo11D::Done => {
                        match hint::match_post_nock(
                            context,
                            subject,
                            dint.tag,
                            Some(dint.hint),
                            dint.body,
                            res,
                        ) {
                            Ok(Some(found)) => res = found,
                            Err(err) => break Err(err),
                            _ => {}
                        }
                        context.stack.pop::<NockWork>();
                    }
                },
                NockWork::Work11S(mut sint) => match sint.todo {
                    Todo11S::ComputeResult => {
                        match hint::match_pre_nock(context, subject, sint.tag, None, sint.body) {
                            Ok(Some(found)) => {
                                res = found;
                                context.stack.pop::<NockWork>();
                            }
                            Ok(None) => {
                                if sint.tail {
                                    context.stack.pop::<NockWork>();
                                } else {
                                    sint.todo = Todo11S::Done;
                                    *context.stack.top() = NockWork::Work11S(sint);
                                }
                                push_formula(&mut context.stack, sint.body, sint.tail)?;
                            }
                            Err(err) => {
                                break Err(err);
                            }
                        }
                    }
                    Todo11S::Done => {
                        match hint::match_post_nock(
                            context, subject, sint.tag, None, sint.body, res,
                        ) {
                            Ok(Some(found)) => res = found,
                            Err(err) => break Err(err),
                            _ => {}
                        }
                        context.stack.pop::<NockWork>();
                    }
                },
                NockWork::Work12(mut scry) => match scry.todo {
                    Todo12::ComputeReff => {
                        let stack = &mut context.stack;
                        scry.todo = Todo12::ComputePath;
                        *stack.top() = NockWork::Work12(scry);
                        push_formula(stack, scry.reff, false)?;
                    }
                    Todo12::ComputePath => {
                        let stack = &mut context.stack;
                        scry.todo = Todo12::Scry;
                        scry.reff = res;
                        *stack.top() = NockWork::Work12(scry);
                        push_formula(stack, scry.path, false)?;
                    }
                    Todo12::Scry => {
                        if let Some(cell) = context.scry_stack.cell() {
                            scry.path = res;
                            let scry_stack = context.scry_stack;
                            let scry_handler = cell.head();
                            let scry_gate = scry_handler.as_cell()?;
                            let payload = T(&mut context.stack, &[scry.reff, res]);
                            let scry_core = T(
                                &mut context.stack,
                                &[
                                    scry_gate.head(),
                                    payload,
                                    scry_gate.tail().as_cell()?.tail(),
                                ],
                            );
                            let scry_form = T(&mut context.stack, &[D(9), D(2), D(1), scry_core]);

                            context.scry_stack = cell.tail();
                            // Alternately, we could use scry_core as the subject and [9 2 0 1] as
                            // the formula. It's unclear if performance will be better with a purely
                            // static formula.
                            match interpret(context, D(0), scry_form) {
                                Ok(noun) => match noun.as_either_atom_cell() {
                                    Left(atom) => {
                                        if atom.as_noun().raw_equals(D(0)) {
                                            break Err(Error::ScryBlocked(scry.path));
                                        } else {
                                            break Err(Error::ScryCrashed(D(0)));
                                        }
                                    }
                                    Right(cell) => match cell.tail().as_either_atom_cell() {
                                        Left(_) => {
                                            let stack = &mut context.stack;
                                            let hunk =
                                                T(stack, &[D(tas!(b"hunk")), scry.reff, scry.path]);
                                            mean_push(stack, hunk);
                                            break Err(Error::ScryCrashed(D(0)));
                                        }
                                        Right(cell) => {
                                            res = cell.tail();
                                            context.scry_stack = scry_stack;
                                            context.stack.pop::<NockWork>();
                                        }
                                    },
                                },
                                Err(error) => match error {
                                    Error::Deterministic(trace) | Error::ScryCrashed(trace) => {
                                        break Err(Error::ScryCrashed(trace));
                                    }
                                    Error::NonDeterministic(_) => {
                                        break Err(error);
                                    }
                                    Error::ScryBlocked(_) => {
                                        break Err(Error::NonDeterministic(D(0)));
                                    }
                                },
                            }
                        } else {
                            // No scry handler
                            break Err(Error::Deterministic(D(0)));
                        }
                    }
                },
            };
        }
    });

    match nock {
        Ok(res) => Ok(res),
        Err(err) => Err(exit(context, virtual_frame, err)),
    }
}

fn push_formula(stack: &mut NockStack, formula: Noun, tail: bool) -> Result<(), Error> {
    unsafe {
        if let Ok(formula_cell) = formula.as_cell() {
            // Formula
            match formula_cell.head().as_either_atom_cell() {
                Right(_cell) => {
                    *stack.push() = NockWork::WorkCons(NockCons {
                        todo: TodoCons::ComputeHead,
                        head: formula_cell.head(),
                        tail: formula_cell.tail(),
                    });
                }
                Left(atom) => {
                    if let Ok(direct) = atom.as_direct() {
                        match direct.data() {
                            0 => {
                                if let Ok(axis_atom) = formula_cell.tail().as_atom() {
                                    *stack.push() = NockWork::Work0(Nock0 { axis: axis_atom });
                                } else {
                                    // Axis for Nock 0 must be an atom
                                    return Err(Error::Deterministic(D(0)));
                                }
                            }
                            1 => {
                                *stack.push() = NockWork::Work1(Nock1 {
                                    noun: formula_cell.tail(),
                                });
                            }
                            2 => {
                                if let Ok(arg_cell) = formula_cell.tail().as_cell() {
                                    *stack.push() = NockWork::Work2(Nock2 {
                                        todo: Todo2::ComputeSubject,
                                        subject: arg_cell.head(),
                                        formula: arg_cell.tail(),
                                        tail,
                                    });
                                } else {
                                    // Argument to Nock 2 must be cell
                                    return Err(Error::Deterministic(D(0)));
                                };
                            }
                            3 => {
                                *stack.push() = NockWork::Work3(Nock3 {
                                    todo: Todo3::ComputeChild,
                                    child: formula_cell.tail(),
                                });
                            }
                            4 => {
                                *stack.push() = NockWork::Work4(Nock4 {
                                    todo: Todo4::ComputeChild,
                                    child: formula_cell.tail(),
                                });
                            }
                            5 => {
                                if let Ok(arg_cell) = formula_cell.tail().as_cell() {
                                    *stack.push() = NockWork::Work5(Nock5 {
                                        todo: Todo5::ComputeLeftChild,
                                        left: arg_cell.head(),
                                        right: arg_cell.tail(),
                                    });
                                } else {
                                    // Argument to Nock 5 must be cell
                                    return Err(Error::Deterministic(D(0)));
                                };
                            }
                            6 => {
                                if let Ok(arg_cell) = formula_cell.tail().as_cell() {
                                    if let Ok(branch_cell) = arg_cell.tail().as_cell() {
                                        *stack.push() = NockWork::Work6(Nock6 {
                                            todo: Todo6::ComputeTest,
                                            test: arg_cell.head(),
                                            zero: branch_cell.head(),
                                            once: branch_cell.tail(),
                                            tail,
                                        });
                                    } else {
                                        // Argument tail to Nock 6 must be cell
                                        return Err(Error::Deterministic(D(0)));
                                    };
                                } else {
                                    // Argument to Nock 6 must be cell
                                    return Err(Error::Deterministic(D(0)));
                                }
                            }
                            7 => {
                                if let Ok(arg_cell) = formula_cell.tail().as_cell() {
                                    *stack.push() = NockWork::Work7(Nock7 {
                                        todo: Todo7::ComputeSubject,
                                        subject: arg_cell.head(),
                                        formula: arg_cell.tail(),
                                        tail,
                                    });
                                } else {
                                    // Argument to Nock 7 must be cell
                                    return Err(Error::Deterministic(D(0)));
                                };
                            }
                            8 => {
                                if let Ok(arg_cell) = formula_cell.tail().as_cell() {
                                    *stack.push() = NockWork::Work8(Nock8 {
                                        todo: Todo8::ComputeSubject,
                                        pin: arg_cell.head(),
                                        formula: arg_cell.tail(),
                                        tail,
                                    });
                                } else {
                                    // Argument to Nock 8 must be cell
                                    return Err(Error::Deterministic(D(0)));
                                };
                            }
                            9 => {
                                if let Ok(arg_cell) = formula_cell.tail().as_cell() {
                                    if let Ok(axis_atom) = arg_cell.head().as_atom() {
                                        *stack.push() = NockWork::Work9(Nock9 {
                                            todo: Todo9::ComputeCore,
                                            axis: axis_atom,
                                            core: arg_cell.tail(),
                                            tail,
                                        });
                                    } else {
                                        // Axis for Nock 9 must be an atom
                                        return Err(Error::Deterministic(D(0)));
                                    }
                                } else {
                                    // Argument to Nock 9 must be cell
                                    return Err(Error::Deterministic(D(0)));
                                };
                            }
                            10 => {
                                if let Ok(arg_cell) = formula_cell.tail().as_cell() {
                                    if let Ok(patch_cell) = arg_cell.head().as_cell() {
                                        if let Ok(axis_atom) = patch_cell.head().as_atom() {
                                            *stack.push() = NockWork::Work10(Nock10 {
                                                todo: Todo10::ComputeTree,
                                                axis: axis_atom,
                                                tree: arg_cell.tail(),
                                                patch: patch_cell.tail(),
                                            });
                                        } else {
                                            // Axis for Nock 10 must be an atom
                                            return Err(Error::Deterministic(D(0)));
                                        }
                                    } else {
                                        // Heah of argument to Nock 10 must be a cell
                                        return Err(Error::Deterministic(D(0)));
                                    };
                                } else {
                                    // Argument to Nock 10 must be a cell
                                    return Err(Error::Deterministic(D(0)));
                                };
                            }
                            11 => {
                                if let Ok(arg_cell) = formula_cell.tail().as_cell() {
                                    match arg_cell.head().as_either_atom_cell() {
                                        Left(tag_atom) => {
                                            *stack.push() = NockWork::Work11S(Nock11S {
                                                todo: Todo11S::ComputeResult,
                                                tag: tag_atom,
                                                body: arg_cell.tail(),
                                                tail: tail && hint::is_tail(tag_atom),
                                            });
                                        }
                                        Right(hint_cell) => {
                                            if let Ok(tag_atom) = hint_cell.head().as_atom() {
                                                *stack.push() = NockWork::Work11D(Nock11D {
                                                    todo: Todo11D::ComputeHint,
                                                    tag: tag_atom,
                                                    hint: hint_cell.tail(),
                                                    body: arg_cell.tail(),
                                                    tail: tail && hint::is_tail(tag_atom),
                                                });
                                            } else {
                                                // Hint tag must be an atom
                                                return Err(Error::Deterministic(D(0)));
                                            }
                                        }
                                    };
                                } else {
                                    // Argument for Nock 11 must be cell
                                    return Err(Error::Deterministic(D(0)));
                                };
                            }
                            12 => {
                                if let Ok(arg_cell) = formula_cell.tail().as_cell() {
                                    *stack.push() = NockWork::Work12(Nock12 {
                                        todo: Todo12::ComputeReff,
                                        reff: arg_cell.head(),
                                        path: arg_cell.tail(),
                                    });
                                } else {
                                    // Argument for Nock 12 must be cell
                                    return Err(Error::Deterministic(D(0)));
                                }
                            }
                            _ => {
                                // Invalid formula opcode
                                return Err(Error::Deterministic(D(0)));
                            }
                        }
                    } else {
                        // Formula opcode must be direct atom
                        return Err(Error::Deterministic(D(0)));
                    }
                }
            }
        } else {
            // Bad formula: atoms are not formulas
            return Err(Error::Deterministic(D(0)));
        }
    }
    Ok(())
}

pub fn exit(context: &mut Context, virtual_frame: *const u64, error: Error) -> Error {
    unsafe {
        let stack = &mut context.stack;

        let mut preserve = match error {
            Error::ScryBlocked(path) => path,
            Error::Deterministic(t) | Error::NonDeterministic(t) | Error::ScryCrashed(t) => {
                // Return $tang of traces
                let h = *(stack.local_noun_pointer(0));
                T(stack, &[h, t])
            }
        };

        while (stack).get_frame_pointer() != virtual_frame {
            (stack).preserve(&mut preserve);
            (stack).frame_pop();
        }

        match error {
            Error::Deterministic(_) => Error::Deterministic(preserve),
            Error::NonDeterministic(_) => Error::NonDeterministic(preserve),
            Error::ScryCrashed(_) => Error::ScryCrashed(preserve),
            Error::ScryBlocked(_) => error,
        }
    }
}

/** Push frame onto NockStack while preserving the mean stack.
 */
fn mean_frame_push(stack: &mut NockStack, slots: usize) {
    unsafe {
        let trace = *(stack.local_noun_pointer(0));
        stack.frame_push(slots + 1);
        *(stack.local_noun_pointer(0)) = trace;
    }
}

/** Push onto the mean stack.
 */
fn mean_push(stack: &mut NockStack, noun: Noun) {
    unsafe {
        let cur_trace = *(stack.local_noun_pointer(0));
        let new_trace = T(stack, &[noun, cur_trace]);
        *(stack.local_noun_pointer(0)) = new_trace;
    }
}

/** Pop off of the mean stack.
 */
fn mean_pop(stack: &mut NockStack) {
    unsafe {
        *(stack.local_noun_pointer(0)) = (*(stack.local_noun_pointer(0)))
            .as_cell()
            .expect("serf: unexpected end of mean stack\r")
            .tail();
    }
}

fn edit(
    stack: &mut NockStack,
    edit_axis: &BitSlice<u64, Lsb0>,
    patch: Noun,
    mut tree: Noun,
) -> Noun {
    let mut res = patch;
    let mut dest: *mut Noun = &mut res;
    let mut cursor = edit_axis
        .last_one()
        .expect("0 is not allowed as an edit axis");
    loop {
        if cursor == 0 {
            unsafe {
                *dest = patch;
            }
            break;
        };
        if let Ok(tree_cell) = tree.as_cell() {
            cursor -= 1;
            if edit_axis[cursor] {
                unsafe {
                    let (cell, cellmem) = Cell::new_raw_mut(stack);
                    *dest = cell.as_noun();
                    (*cellmem).head = tree_cell.head();
                    dest = &mut ((*cellmem).tail);
                }
                tree = tree_cell.tail();
            } else {
                unsafe {
                    let (cell, cellmem) = Cell::new_raw_mut(stack);
                    *dest = cell.as_noun();
                    (*cellmem).tail = tree_cell.tail();
                    dest = &mut ((*cellmem).head);
                }
                tree = tree_cell.head();
            }
        } else {
            panic!("Invalid axis for edit");
        };
    }
    res
}

pub fn inc(stack: &mut NockStack, atom: Atom) -> Atom {
    match atom.as_either() {
        Left(direct) => Atom::new(stack, direct.data() + 1),
        Right(indirect) => {
            let indirect_slice = indirect.as_bitslice();
            match indirect_slice.first_zero() {
                None => {
                    // all ones, make an indirect one word bigger
                    let (new_indirect, new_slice) =
                        unsafe { IndirectAtom::new_raw_mut_bitslice(stack, indirect.size() + 1) };
                    new_slice.set(indirect_slice.len(), true);
                    new_indirect.as_atom()
                }
                Some(first_zero) => {
                    let (new_indirect, new_slice) =
                        unsafe { IndirectAtom::new_raw_mut_bitslice(stack, indirect.size()) };
                    new_slice.set(first_zero, true);
                    new_slice[first_zero + 1..]
                        .copy_from_bitslice(&indirect_slice[first_zero + 1..]);
                    new_indirect.as_atom()
                }
            }
        }
    }
}

mod hint {
    use super::*;
    use crate::jets;
    use crate::jets::cold;
    use crate::jets::nock::util::{mook, LEAF};
    use crate::mem::unifying_equality;
    use crate::noun::{tape, Atom, Cell, Noun, D, T};
    use crate::serf::TERMINATOR;
    use ares_macros::tas;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;

    pub fn is_tail(tag: Atom) -> bool {
        //  XX: handle IndirectAtom tags
        match tag.direct() {
            #[allow(clippy::match_like_matches_macro)]
            Some(dtag) => match dtag.data() {
                tas!(b"fast") => false,
                tas!(b"memo") => false,
                _ => true,
            },
            None => true,
        }
    }

    /** Match dynamic hints before the hint formula is evaluated */
    pub fn match_pre_hint(
        context: &mut Context,
        subject: Noun,
        tag: Atom,
        hint: Noun,
        body: Noun,
    ) -> Result<Option<Noun>, Error> {
        //  XX: handle IndirectAtom tags
        match tag.as_direct()?.data() {
            tas!(b"sham") => {
                if cfg!(feature = "sham_hints") {
                    let jet_formula = hint.as_cell()?;
                    // XX: what is the head here?
                    let jet_name = jet_formula.tail();

                    if let Some(jet) = jets::get_jet(jet_name) {
                        match jet(context, subject) {
                            Ok(mut jet_res) => {
                                //  XX: simplify this by moving jet test mode into the 11 code in interpret, or into its own function?
                                // if in test mode, check that the jet returns the same result as the raw nock
                                if jets::get_jet_test_mode(jet_name) {
                                    //  XX: we throw away trace, which might matter for non-deterministic errors
                                    //      maybe mook and slog it?
                                    match interpret(context, subject, body) {
                                        Ok(mut nock_res) => {
                                            let stack = &mut context.stack;
                                            if unsafe {
                                                !unifying_equality(
                                                    stack,
                                                    &mut nock_res,
                                                    &mut jet_res,
                                                )
                                            } {
                                                //  XX: need string interpolation without allocation, then delete eprintln
                                                // let tape = tape(stack, "jet mismatch in {}, raw: {}, jetted: {}", jet_name, nock_res, jet_res);
                                                eprintln!(
                                                    "\rjet {} failed, raw: {:?}, jetted: {}",
                                                    jet_name, nock_res, jet_res
                                                );
                                                let tape = tape(stack, "jet mismatch");
                                                let mean = T(stack, &[D(tas!(b"mean")), tape]);
                                                mean_push(stack, mean);
                                                Err(Error::Deterministic(D(0)))
                                            } else {
                                                Ok(Some(nock_res))
                                            }
                                        }
                                        Err(error) => {
                                            let stack = &mut context.stack;
                                            //  XX: need string interpolation without allocation, then delete eprintln
                                            // let tape = tape(stack, "jet mismatch in {}, raw: {}, jetted: {}", jet_name, err, jet_res);
                                            eprintln!(
                                                "\rjet {} failed, raw: {:?}, jetted: {}",
                                                jet_name, error, jet_res
                                            );
                                            let tape = tape(stack, "jet mismatch");
                                            let mean = T(stack, &[D(tas!(b"mean")), tape]);
                                            mean_push(stack, mean);

                                            match error {
                                                Error::NonDeterministic(_) => {
                                                    Err(Error::NonDeterministic(D(0)))
                                                }
                                                _ => Err(Error::Deterministic(D(0))),
                                            }
                                        }
                                    }
                                } else {
                                    Ok(Some(jet_res))
                                }
                            }
                            Err(JetErr::Punt) => Ok(None),
                            Err(err) => {
                                let stack = &mut context.stack;
                                //  XX: need string interpolation without allocation
                                // let tape = tape(stack, "{} jet error in {}", err, jet_name);
                                let tape = tape(stack, "jet error");
                                let mean = T(stack, &[D(tas!(b"mean")), tape]);
                                mean_push(stack, mean);
                                Err(err.into())
                            }
                        }
                    } else {
                        Ok(None)
                    }
                } else {
                    Ok(None)
                }
            }
            tas!(b"memo") => {
                let stack = &mut context.stack;
                let mut key = Cell::new(stack, subject, body).as_noun();
                Ok(context.cache.lookup(stack, &mut key))
            }
            _ => Ok(None),
        }
    }

    /** Match static and dynamic hints before the nock formula is evaluated */
    pub fn match_pre_nock(
        context: &mut Context,
        _subject: Noun,
        tag: Atom,
        hint: Option<(Noun, Noun)>,
        _body: Noun,
    ) -> Result<Option<Noun>, Error> {
        //  XX: handle IndirectAtom tags
        match tag.as_direct()?.data() {
            tas!(b"slog") => {
                let stack = &mut context.stack;
                let newt = &mut context.newt;

                let (_form, clue) = hint.ok_or(Error::Deterministic(D(0)))?;
                let slog_cell = clue.as_cell()?;
                let pri = slog_cell.head().as_direct()?.data();
                let tank = slog_cell.tail();

                newt.slog(stack, pri, tank);
                Ok(None)
            }
            tas!(b"hand") | tas!(b"hunk") | tas!(b"lose") | tas!(b"mean") | tas!(b"spot") => {
                let terminator = Arc::clone(&TERMINATOR);
                if (*terminator).load(Ordering::Relaxed) {
                    return Err(Error::NonDeterministic(D(0)));
                }

                let stack = &mut context.stack;
                let (_form, clue) = hint.ok_or(Error::Deterministic(D(0)))?;
                let noun = T(stack, &[tag.as_noun(), clue]);
                mean_push(stack, noun);
                Ok(None)
            }
            tas!(b"hela") => {
                //  XX: This only prints the trace down to the bottom of THIS
                //      interpret call. We'll need to recursively work down
                //      frames to get the stack trace all the way to the root.
                let mean = unsafe { *(context.stack.local_noun_pointer(0)) };
                let tone = Cell::new(&mut context.stack, D(2), mean);

                match mook(context, tone, true) {
                    Ok(toon) => {
                        let stack = &mut context.stack;
                        let newt = &mut context.newt;

                        if unsafe { !toon.head().raw_equals(D(2)) } {
                            let tape = tape(stack, "%hela failed: toon not %2");
                            let mean = T(stack, &[D(tas!(b"mean")), tape]);
                            mean_push(stack, mean);
                            return Err(Error::Deterministic(D(0)));
                        }

                        let mut list = toon.tail();
                        loop {
                            if unsafe { list.raw_equals(D(0)) } {
                                break;
                            }

                            let cell = list.as_cell().unwrap();
                            newt.slog(stack, 0, cell.head());

                            list = cell.tail();
                        }

                        Ok(None)
                    }
                    Err(err) => {
                        let stack = &mut context.stack;
                        let tape = tape(stack, "%hela failed: mook error");
                        let mean = T(stack, &[D(tas!(b"mean")), tape]);
                        mean_push(stack, mean);
                        Err(err.into())
                    }
                }
            }
            _ => Ok(None),
        }
    }

    /** Match static and dynamic hints after the nock formula is evaluated */
    pub fn match_post_nock(
        context: &mut Context,
        subject: Noun,
        tag: Atom,
        hint: Option<Noun>,
        body: Noun,
        res: Noun,
    ) -> Result<Option<Noun>, Error> {
        let stack = &mut context.stack;
        let newt = &mut context.newt;
        let cold = &mut context.cold;
        let hot = &context.hot;
        let cache = &mut context.cache;

        //  XX: handle IndirectAtom tags
        match tag.as_direct()?.data() {
            tas!(b"memo") => {
                let mut key = Cell::new(stack, subject, body).as_noun();
                context.cache = cache.insert(stack, &mut key, res);
            }
            tas!(b"hand") | tas!(b"hunk") | tas!(b"lose") | tas!(b"mean") | tas!(b"spot") => {
                mean_pop(stack);
            }
            tas!(b"fast") => {
                if !cfg!(feature = "sham_hints") {
                    if let Some(clue) = hint {
                        let cold_res: cold::Result = {
                            let chum = clue.slot(2)?;
                            let parent_formula_op = clue.slot(12)?.as_atom()?.as_direct()?;
                            let parent_formula_ax = clue.slot(13)?.as_atom()?;

                            if parent_formula_op.data() == 1 {
                                if parent_formula_ax.as_direct()?.data() == 0 {
                                    cold.register(stack, res, parent_formula_ax, chum)
                                } else {
                                    //  XX: Need better message in slog; need better slogging tools
                                    //      format!("invalid root parent axis: {} {}", chum, parent_formula_ax)
                                    let tape = tape(
                                        stack,
                                        "serf: cold: register: invalid root parent axis",
                                    );
                                    slog_leaf(stack, newt, tape);
                                    Ok(false)
                                }
                            } else {
                                cold.register(stack, res, parent_formula_ax, chum)
                            }
                        };

                        match cold_res {
                            Ok(true) => context.warm = Warm::init(stack, cold, hot),
                            Err(cold::Error::NoParent) => {
                                //  XX: Need better message in slog; need better slogging tools
                                //      format!("could not find parent battery at given axis: {} {}", chum, parent_formula_ax)
                                let tape = tape(
                                    stack,
                                    "serf: cold: register: could not find parent battery at given axis",
                                );
                                slog_leaf(stack, newt, tape);
                            }
                            Err(cold::Error::BadNock) => {
                                //  XX: Need better message in slog; need better slogging tools
                                //      format!("bad clue formula: {}", clue)
                                let tape = tape(stack, "serf: cold: register: bad clue formula");
                                slog_leaf(stack, newt, tape);
                            }
                            _ => {}
                        }
                    } else {
                        let tape = tape(stack, "serf: cold: register: no clue for %fast");
                        slog_leaf(stack, newt, tape);
                    }
                }
            }
            _ => {}
        }

        Ok(None)
    }

    fn slog_leaf(stack: &mut NockStack, newt: &mut Newt, tape: Noun) {
        let tank = T(stack, &[LEAF, tape]);
        newt.slog(stack, 0u64, tank);
    }
}
