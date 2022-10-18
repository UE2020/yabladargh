use font_kit::font::Font;
use image::RgbaImage;
use raqote::*;
use std::cell::RefCell;
use v8::{FunctionTemplate, HandleScope, Local, ObjectTemplate, TryCatch};

use font_kit::family_name::FamilyName;
use font_kit::properties::Properties;
use font_kit::source::SystemSource;

thread_local! {
    pub static IMAGE: RefCell<Option<RgbaImage>> = RefCell::new(None);
    pub static DT: RefCell<Option<DrawTarget>> = RefCell::new(None);
    pub static CURRENT_PATH: RefCell<Option<PathBuilder>> = RefCell::new(None);
    pub static FINISHED_PATH: RefCell<Option<Path>> = RefCell::new(None);
    pub static DEFAULT_FONT: Font = SystemSource::new()
        .select_best_match(&[FamilyName::SansSerif], &Properties::new())
        .unwrap()
        .load()
        .unwrap();
}

fn as_u8_slice(v: &[u32]) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            v.as_ptr() as *const u8,
            v.len() * std::mem::size_of::<u32>(),
        )
    }
}

pub fn populate_global(global: &Local<ObjectTemplate>, scope: &mut HandleScope<()>) {
    IMAGE.with(|f| {
        *f.borrow_mut() = None;
    });

    DT.with(|f| {
        *f.borrow_mut() = None;
    });

    CURRENT_PATH.with(|f| {
        *f.borrow_mut() = None;
    });

    FINISHED_PATH.with(|f| {
        *f.borrow_mut() = None;
    });

    global.set(
        v8::String::new(scope, "setImage").unwrap().into(),
        v8::FunctionTemplate::new(
            scope,
            |scope: &mut v8::HandleScope,
             args: v8::FunctionCallbackArguments,
             mut _retval: v8::ReturnValue| {
                let arg = args.get(0);
                let w = args.get(1).to_number(scope).unwrap().value() as u32;
                let h = args.get(2).to_number(scope).unwrap().value() as u32;
                if w == 0 || h == 0 {
                    let err = v8::String::new(scope, "Zero dimension not allowed")
                        .unwrap()
                        .into();
                    scope.throw_exception(err);
                }
                if arg.is_uint8_array() {
                    // SAFETY: THIS IS A UINT8 ARRAY
                    let buf: v8::Local<v8::Uint8Array> = unsafe { v8::Local::cast(arg) };
                    let mut copied = vec![0; buf.byte_length()];
                    buf.copy_contents(&mut copied);
                    IMAGE.with(|f| {
                        *f.borrow_mut() = RgbaImage::from_raw(w, h, copied);
                    });
                } else {
                    IMAGE.with(|f| {
                        *f.borrow_mut() = None;
                    });
                }
            },
        )
        .into(),
    );

    global.set(
        v8::String::new(scope, "isStupid").unwrap().into(),
        v8::FunctionTemplate::new(
            scope,
            |scope: &mut v8::HandleScope,
             args: v8::FunctionCallbackArguments,
             mut retval: v8::ReturnValue| {
                let arg = args
                    .get(0)
                    .to_string(scope)
                    .unwrap()
                    .to_rust_string_lossy(scope);
                retval.set_bool(&arg == "Steve-Dusty");
            },
        )
        .into(),
    );

    global.set(
        v8::String::new(scope, "newCanvas").unwrap().into(),
        v8::FunctionTemplate::new(
            scope,
            |scope: &mut v8::HandleScope,
             args: v8::FunctionCallbackArguments,
             mut retval: v8::ReturnValue| {
                let w = args.get(0).to_number(scope).unwrap().value() as u32;
                let h = args.get(1).to_number(scope).unwrap().value() as u32;
                if w == 0 || h == 0 {
                    let err = v8::String::new(scope, "Zero dimension not allowed")
                        .unwrap()
                        .into();
                    scope.throw_exception(err);
                }

                DT.with(|f| {
                    *f.borrow_mut() = Some(DrawTarget::new(w as i32, h as i32));
                });
            },
        )
        .into(),
    );

    global.set(
        v8::String::new(scope, "fillRect").unwrap().into(),
        v8::FunctionTemplate::new(
            scope,
            |scope: &mut v8::HandleScope,
             args: v8::FunctionCallbackArguments,
             mut retval: v8::ReturnValue| {
                let x = args.get(0).to_number(scope).unwrap().value() as f32;
                let y = args.get(1).to_number(scope).unwrap().value() as f32;
                let w = args.get(2).to_number(scope).unwrap().value() as f32;
                let h = args.get(3).to_number(scope).unwrap().value() as f32;

                let r = args.get(4).to_number(scope).unwrap().value() as u8;
                let g = args.get(5).to_number(scope).unwrap().value() as u8;
                let b = args.get(6).to_number(scope).unwrap().value() as u8;
                let a = args.get(7).to_number(scope).unwrap().value() as u8;

                DT.with(|f| {
                    let mut f = f.borrow_mut();
                    if let Some(ctx) = &mut *f {
                        ctx.fill_rect(
                            x,
                            y,
                            w,
                            h,
                            &Source::Solid(SolidSource { r, g, b, a }),
                            &DrawOptions::new(),
                        );
                    } else {
                        let err = v8::String::new(scope, "Canvas not initialized")
                            .unwrap()
                            .into();
                        scope.throw_exception(err);
                    }
                });
            },
        )
        .into(),
    );

    global.set(
        v8::String::new(scope, "drawText").unwrap().into(),
        v8::FunctionTemplate::new(
            scope,
            |scope: &mut v8::HandleScope,
             args: v8::FunctionCallbackArguments,
             mut retval: v8::ReturnValue| {
                let size = args.get(0).to_number(scope).unwrap().value() as f32;
                let text = args.get(1).to_string(scope).unwrap().to_rust_string_lossy(scope);
                let x = args.get(2).to_number(scope).unwrap().value() as f32;
                let y = args.get(3).to_number(scope).unwrap().value() as f32;

                let r = args.get(4).to_number(scope).unwrap().value() as u8;
                let g = args.get(5).to_number(scope).unwrap().value() as u8;
                let b = args.get(6).to_number(scope).unwrap().value() as u8;
                let a = args.get(7).to_number(scope).unwrap().value() as u8;

                DT.with(|f| {
                    let mut f = f.borrow_mut();
                    if let Some(ctx) = &mut *f {
                        DEFAULT_FONT.with(|font| {
                            ctx.draw_text(
                                font,
                                size,
                                &text,
                                Point::new(x, y),
                                &Source::Solid(SolidSource { r, g, b, a }),
                                &DrawOptions::new(),
                            );
                        });
                    } else {
                        let err = v8::String::new(scope, "Canvas not initialized")
                            .unwrap()
                            .into();
                        scope.throw_exception(err);
                    }
                });
            },
        )
        .into(),
    );

    global.set(
        v8::String::new(scope, "setImageToCanvas").unwrap().into(),
        v8::FunctionTemplate::new(
            scope,
            |scope: &mut v8::HandleScope,
             args: v8::FunctionCallbackArguments,
             mut retval: v8::ReturnValue| {
                DT.with(|f| {
                    let mut f = f.borrow_mut();
                    let ctx = f.take();
                    if let Some(ctx) = ctx {
                        let w = ctx.width();
                        let h = ctx.height();
                        let raw_data = ctx.into_vec();
                        let casted = as_u8_slice(&raw_data);
                        IMAGE.with(|f| {
                            *f.borrow_mut() =
                                RgbaImage::from_raw(w as u32, h as u32, casted.to_vec());
                        });
                    } else {
                        let err = v8::String::new(scope, "Canvas not initialized")
                            .unwrap()
                            .into();
                        scope.throw_exception(err);
                    }
                });
            },
        )
        .into(),
    );

    global.set(
        v8::String::new(scope, "beginPath").unwrap().into(),
        v8::FunctionTemplate::new(
            scope,
            |scope: &mut v8::HandleScope,
             args: v8::FunctionCallbackArguments,
             mut retval: v8::ReturnValue| {
                CURRENT_PATH.with(|f| {
                    let mut f = f.borrow_mut();
                    *f = Some(PathBuilder::new());
                });
            },
        )
        .into(),
    );

    global.set(
        v8::String::new(scope, "moveTo").unwrap().into(),
        v8::FunctionTemplate::new(
            scope,
            |scope: &mut v8::HandleScope,
             args: v8::FunctionCallbackArguments,
             mut retval: v8::ReturnValue| {
                let x = args.get(0).to_number(scope).unwrap().value() as f32;
                let y = args.get(1).to_number(scope).unwrap().value() as f32;
                CURRENT_PATH.with(|f| {
                    let mut f = f.borrow_mut();
                    if let Some(path) = &mut *f {
                        path.move_to(x, y);
                    } else {
                        let err = v8::String::new(scope, "Path not initialized")
                            .unwrap()
                            .into();
                        scope.throw_exception(err);
                    }
                });
            },
        )
        .into(),
    );

    global.set(
        v8::String::new(scope, "cubicTo").unwrap().into(),
        v8::FunctionTemplate::new(
            scope,
            |scope: &mut v8::HandleScope,
             args: v8::FunctionCallbackArguments,
             mut retval: v8::ReturnValue| {
                let x = args.get(0).to_number(scope).unwrap().value() as f32;
                let y = args.get(1).to_number(scope).unwrap().value() as f32;
                let x2 = args.get(2).to_number(scope).unwrap().value() as f32;
                let y2 = args.get(3).to_number(scope).unwrap().value() as f32;
                let x3 = args.get(4).to_number(scope).unwrap().value() as f32;
                let y3 = args.get(5).to_number(scope).unwrap().value() as f32;
                CURRENT_PATH.with(|f| {
                    let mut f = f.borrow_mut();
                    if let Some(path) = &mut *f {
                        path.cubic_to(x, y, x2, y2, x3, y3);
                    } else {
                        let err = v8::String::new(scope, "Path not initialized")
                            .unwrap()
                            .into();
                        scope.throw_exception(err);
                    }
                });
            },
        )
        .into(),
    );

    global.set(
        v8::String::new(scope, "quadTo").unwrap().into(),
        v8::FunctionTemplate::new(
            scope,
            |scope: &mut v8::HandleScope,
             args: v8::FunctionCallbackArguments,
             mut retval: v8::ReturnValue| {
                let x = args.get(0).to_number(scope).unwrap().value() as f32;
                let y = args.get(1).to_number(scope).unwrap().value() as f32;
                let x2 = args.get(2).to_number(scope).unwrap().value() as f32;
                let y2 = args.get(3).to_number(scope).unwrap().value() as f32;
                CURRENT_PATH.with(|f| {
                    let mut f = f.borrow_mut();
                    if let Some(path) = &mut *f {
                        path.quad_to(x, y, x2, y2);
                    } else {
                        let err = v8::String::new(scope, "Path not initialized")
                            .unwrap()
                            .into();
                        scope.throw_exception(err);
                    }
                });
            },
        )
        .into(),
    );

    global.set(
        v8::String::new(scope, "arc").unwrap().into(),
        v8::FunctionTemplate::new(
            scope,
            |scope: &mut v8::HandleScope,
             args: v8::FunctionCallbackArguments,
             mut retval: v8::ReturnValue| {
                let x = args.get(0).to_number(scope).unwrap().value() as f32;
                let y = args.get(1).to_number(scope).unwrap().value() as f32;
                let r = args.get(2).to_number(scope).unwrap().value() as f32;
                let start_angle = args.get(3).to_number(scope).unwrap().value() as f32;
                let sweep_angle = args.get(4).to_number(scope).unwrap().value() as f32;
                CURRENT_PATH.with(|f| {
                    let mut f = f.borrow_mut();
                    if let Some(path) = &mut *f {
                        path.arc(x, y, r, start_angle, sweep_angle);
                    } else {
                        let err = v8::String::new(scope, "Path not initialized")
                            .unwrap()
                            .into();
                        scope.throw_exception(err);
                    }
                });
            },
        )
        .into(),
    );

    global.set(
        v8::String::new(scope, "closePath").unwrap().into(),
        v8::FunctionTemplate::new(
            scope,
            |scope: &mut v8::HandleScope,
             args: v8::FunctionCallbackArguments,
             mut retval: v8::ReturnValue| {
                CURRENT_PATH.with(|f| {
                    let mut f = f.borrow_mut();
                    if let Some(path) = &mut *f {
                        path.close();
                    } else {
                        let err = v8::String::new(scope, "Path not initialized")
                            .unwrap()
                            .into();
                        scope.throw_exception(err);
                    }
                });
            },
        )
        .into(),
    );

    global.set(
        v8::String::new(scope, "finishPath").unwrap().into(),
        v8::FunctionTemplate::new(
            scope,
            |scope: &mut v8::HandleScope,
             args: v8::FunctionCallbackArguments,
             mut retval: v8::ReturnValue| {
                CURRENT_PATH.with(|f| {
                    let mut f = f.borrow_mut();
                    let mut f = f.take();
                    if let Some(path) = f {
                        FINISHED_PATH.with(|f| {
                            let path = path.finish();
                            let mut f = f.borrow_mut();
                            *f = Some(path);
                        });
                    } else {
                        let err = v8::String::new(scope, "Path not initialized")
                            .unwrap()
                            .into();
                        scope.throw_exception(err);
                    }
                });
            },
        )
        .into(),
    );

    global.set(
        v8::String::new(scope, "fillPath").unwrap().into(),
        v8::FunctionTemplate::new(
            scope,
            |scope: &mut v8::HandleScope,
             args: v8::FunctionCallbackArguments,
             mut retval: v8::ReturnValue| {
                let r = args.get(0).to_number(scope).unwrap().value() as u8;
                let g = args.get(1).to_number(scope).unwrap().value() as u8;
                let b = args.get(2).to_number(scope).unwrap().value() as u8;
                let a = args.get(3).to_number(scope).unwrap().value() as u8;

                DT.with(|f| {
                    let mut f = f.borrow_mut();
                    if let Some(ctx) = &mut *f {
                        FINISHED_PATH.with(|f| {
                            let f = f.borrow();
                            match &*f {
                                Some(path) => ctx.fill(
                                    path,
                                    &Source::Solid(SolidSource { r, g, b, a }),
                                    &DrawOptions::new(),
                                ),
                                None => {
                                    let err =
                                        v8::String::new(scope, "No path to draw").unwrap().into();
                                    scope.throw_exception(err);
                                }
                            }
                        });
                    } else {
                        let err = v8::String::new(scope, "Canvas not initialized")
                            .unwrap()
                            .into();
                        scope.throw_exception(err);
                    }
                });
            },
        )
        .into(),
    );
}

pub fn get_exception(mut scope: TryCatch<HandleScope>) -> String {
    let exception = scope.exception().unwrap();
    let exception_string = exception
        .to_string(&mut scope)
        .unwrap()
        .to_rust_string_lossy(&mut scope);
    let message = if let Some(message) = scope.message() {
        message
    } else {
        return exception_string;
    };
    let filename = message.get_script_resource_name(&mut scope).map_or_else(
        || "(unknown)".into(),
        |s| {
            s.to_string(&mut scope)
                .unwrap()
                .to_rust_string_lossy(&mut scope)
        },
    );
    let line_number = message.get_line_number(&mut scope).unwrap_or_default();
    format!("{}:{}: {}", filename, line_number - 1, exception_string)
}
