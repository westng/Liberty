use super::*;
use objc2::{define_class, msg_send, AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSColor, NSEvent, NSFloatingWindowLevel, NSFont, NSImage, NSImageScaling, NSImageView,
    NSTextAlignment, NSTextField, NSView, NSWindow, NSWindowCollectionBehavior,
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::NSString;
use objc2_quartz_core::CALayer;
use std::path::Path;

const BUBBLE_LAYER_NAME: &str = "LibertyDesktopPetBubbleLayer";
const GROWTH_LABEL_TAG: isize = 8102;

define_class!(
    #[unsafe(super(NSView))]
    #[name = "LibertyDesktopPetContentView"]
    #[thread_kind = MainThreadOnly]
    #[ivars = Arc<AtomicU64>]
    struct PetContentView;

    impl PetContentView {
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            self.ivars().fetch_add(1, Ordering::Relaxed);
            if let Some(window) = event.window(MainThreadMarker::from(self)) {
                window.performWindowDragWithEvent(event);
            }
        }
    }
);

unsafe impl objc2_foundation::NSObjectProtocol for PetContentView {}

impl PetContentView {
    fn new(
        mtm: MainThreadMarker,
        frame: CGRect,
        interaction_signal: Arc<AtomicU64>,
    ) -> objc2::rc::Retained<Self> {
        let this = mtm.alloc().set_ivars(interaction_signal);
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

pub fn prepare_window(window: &Window, interaction_signal: Arc<AtomicU64>) -> LocalResult<()> {
    let mtm =
        MainThreadMarker::new().ok_or_else(|| "macOS 桌宠初始化必须在主线程执行。".to_string())?;
    let ns_window = native_window(window)?;
    ns_window.setOpaque(false);
    ns_window.setBackgroundColor(Some(&NSColor::clearColor()));
    ns_window.setAlphaValue(1.0);
    ns_window.setLevel(NSFloatingWindowLevel);
    ns_window.setMovable(true);
    ns_window.setMovableByWindowBackground(true);
    ns_window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::Transient,
    );
    let content_view = PetContentView::new(
        mtm,
        CGRect::new(
            CGPoint::new(0.0, 0.0),
            CGSize::new(PET_WINDOW_WIDTH, PET_WINDOW_HEIGHT),
        ),
        interaction_signal,
    );
    content_view.setWantsLayer(true);
    ns_window.setContentView(Some(&content_view));
    ns_window.setIgnoresMouseEvents(false);
    ns_window.orderFrontRegardless();
    Ok(())
}

pub fn paint_window(
    window: &Window,
    frame_path: &Path,
    bubble_text: Option<&str>,
    growth_float: Option<&PetGrowthFloat>,
    bubble_theme: PetBubbleTheme,
) -> LocalResult<()> {
    let mtm =
        MainThreadMarker::new().ok_or_else(|| "macOS 桌宠绘制必须在主线程执行。".to_string())?;
    let ns_window = native_window(window)?;
    let frame_path = frame_path
        .to_str()
        .ok_or_else(|| format!("宠物图片路径不是有效 UTF-8：{}", frame_path.display()))?;
    let image_path = NSString::from_str(frame_path);
    let image = NSImage::initWithContentsOfFile(NSImage::alloc(), &image_path)
        .ok_or_else(|| format!("读取宠物图片失败：{frame_path}"))?;

    ns_window.setOpaque(false);
    ns_window.setBackgroundColor(Some(&NSColor::clearColor()));

    let content_view = ns_window
        .contentView()
        .ok_or_else(|| "macOS 桌宠内容视图为空。".to_string())?;
    let image_view = match content_view
        .subviews()
        .into_iter()
        .find_map(|view| view.downcast::<NSImageView>().ok())
    {
        Some(image_view) => image_view,
        None => {
            let view = create_image_view(mtm);
            content_view.addSubview(&view);
            view
        }
    };

    image_view.setFrame(CGRect::new(
        CGPoint::new((PET_WINDOW_WIDTH - PET_SPRITE_WIDTH as f64) / 2.0, 6.0),
        CGSize::new(PET_SPRITE_WIDTH as f64, PET_SPRITE_HEIGHT as f64),
    ));
    image_view.setImage(Some(&image));

    update_bubble_label(mtm, &content_view, bubble_text, bubble_theme);
    update_growth_label(mtm, &content_view, growth_float);

    image_view.displayIfNeeded();
    content_view.setNeedsDisplay(true);
    ns_window.displayIfNeeded();

    Ok(())
}

fn update_growth_label(
    mtm: MainThreadMarker,
    content_view: &NSView,
    growth_float: Option<&PetGrowthFloat>,
) {
    let label = match content_view
        .subviews()
        .into_iter()
        .filter_map(|view| view.downcast::<NSTextField>().ok())
        .find(|label| label.tag() == GROWTH_LABEL_TAG)
    {
        Some(label) => label,
        None => {
            let initial = NSString::from_str("");
            let label = NSTextField::labelWithString(&initial, mtm);
            label.setTag(GROWTH_LABEL_TAG);
            label.setDrawsBackground(false);
            label.setBordered(false);
            label.setBezeled(false);
            label.setEditable(false);
            label.setSelectable(false);
            label.setAlignment(NSTextAlignment::Center);
            label.setFont(Some(&NSFont::boldSystemFontOfSize(18.0)));
            content_view.addSubview(&label);
            label
        }
    };

    let Some(growth_float) = growth_float else {
        label.setHidden(true);
        return;
    };
    let elapsed_ms = growth_float
        .started_at
        .elapsed()
        .unwrap_or_default()
        .as_millis()
        .min(3_000) as f64;
    let progress = elapsed_ms / 3_000.0;
    let y = 88.0 + progress * 34.0;
    let alpha = if progress < 0.72 {
        1.0
    } else {
        ((1.0 - progress) / 0.28).clamp(0.0, 1.0)
    };
    let text = NSString::from_str(&format!("+{}", growth_float.value));
    label.setStringValue(&text);
    label.setTextColor(Some(&NSColor::colorWithCalibratedRed_green_blue_alpha(
        1.0, 0.38, 0.22, alpha,
    )));
    label.setFrame(CGRect::new(CGPoint::new(196.0, y), CGSize::new(78.0, 28.0)));
    label.setHidden(false);
}

fn create_image_view(mtm: MainThreadMarker) -> objc2::rc::Retained<NSImageView> {
    let view = NSImageView::initWithFrame(
        NSImageView::alloc(mtm),
        CGRect::new(
            CGPoint::new((PET_WINDOW_WIDTH - PET_SPRITE_WIDTH as f64) / 2.0, 6.0),
            CGSize::new(PET_SPRITE_WIDTH as f64, PET_SPRITE_HEIGHT as f64),
        ),
    );
    view.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
    view
}

fn update_bubble_label(
    mtm: MainThreadMarker,
    content_view: &NSView,
    bubble_text: Option<&str>,
    bubble_theme: PetBubbleTheme,
) {
    let label = match content_view
        .subviews()
        .into_iter()
        .find_map(|view| view.downcast::<NSTextField>().ok())
    {
        Some(label) => label,
        None => {
            let initial = NSString::from_str("");
            let label = NSTextField::wrappingLabelWithString(&initial, mtm);
            label.setDrawsBackground(false);
            label.setBordered(false);
            label.setBezeled(false);
            label.setEditable(false);
            label.setSelectable(false);
            label.setWantsLayer(true);
            label.setMaximumNumberOfLines(2);
            label.setAlignment(NSTextAlignment::Center);
            label.setFont(Some(&NSFont::systemFontOfSize(13.5)));
            content_view.addSubview(&label);
            label
        }
    };
    let (background, foreground) = if bubble_theme.is_dark() {
        (
            NSColor::colorWithCalibratedWhite_alpha(0.10, 1.0),
            NSColor::colorWithCalibratedWhite_alpha(0.94, 1.0),
        )
    } else {
        (
            NSColor::colorWithCalibratedWhite_alpha(1.0, 1.0),
            NSColor::colorWithCalibratedWhite_alpha(0.10, 1.0),
        )
    };
    label.setDrawsBackground(false);
    label.setBackgroundColor(Some(&NSColor::clearColor()));
    label.setTextColor(Some(&foreground));

    if let Some(text) = bubble_text.filter(|value| !value.trim().is_empty()) {
        apply_bubble_layer_style(content_view, &background, bubble_theme);
        if let Some(label_layer) = label.layer() {
            label_layer.setZPosition(2.0);
        }

        let text = NSString::from_str(text);
        label.setStringValue(&text);
        label.setFrame(text_frame_for_bubble(text.length()));
        label.setHidden(false);
    } else {
        label.setHidden(true);
        set_bubble_layer_hidden(content_view, true);
    }
}

fn text_frame_for_bubble(character_count: usize) -> CGRect {
    let line_count = if character_count > 16 { 2.0 } else { 1.0 };
    let line_height = 17.0;
    let text_height = line_count * line_height;
    let bubble_y = 156.0;
    let bubble_height = 58.0;
    CGRect::new(
        CGPoint::new(34.0, bubble_y + (bubble_height - text_height) / 2.0),
        CGSize::new(PET_WINDOW_WIDTH - 68.0, text_height),
    )
}

fn apply_bubble_layer_style(
    content_view: &NSView,
    background: &NSColor,
    bubble_theme: PetBubbleTheme,
) {
    content_view.setWantsLayer(true);
    let Some(root_layer) = content_view.layer() else {
        return;
    };
    let layer = find_or_create_bubble_layer(&root_layer);
    let border = if bubble_theme.is_dark() {
        NSColor::colorWithCalibratedWhite_alpha(1.0, 0.14)
    } else {
        NSColor::colorWithCalibratedWhite_alpha(0.0, 0.10)
    };
    let shadow = NSColor::colorWithCalibratedWhite_alpha(
        0.0,
        if bubble_theme.is_dark() { 0.34 } else { 0.18 },
    );
    layer.setFrame(CGRect::new(
        CGPoint::new(20.0, 156.0),
        CGSize::new(PET_WINDOW_WIDTH - 40.0, 58.0),
    ));
    layer.setZPosition(1.0);
    layer.setBackgroundColor(Some(&background.CGColor()));
    layer.setCornerRadius(14.0);
    layer.setMasksToBounds(false);
    layer.setBorderWidth(1.0);
    layer.setBorderColor(Some(&border.CGColor()));
    layer.setShadowColor(Some(&shadow.CGColor()));
    layer.setShadowOpacity(if bubble_theme.is_dark() { 0.30 } else { 0.18 });
    layer.setShadowRadius(10.0);
    layer.setShadowOffset(CGSize::new(0.0, -2.0));
    layer.setHidden(false);
}

fn set_bubble_layer_hidden(content_view: &NSView, hidden: bool) {
    let Some(root_layer) = content_view.layer() else {
        return;
    };
    let layer_name = NSString::from_str(BUBBLE_LAYER_NAME);
    if let Some(layer) = unsafe { root_layer.sublayers() }.and_then(|layers| {
        layers.into_iter().find(|layer| {
            layer
                .name()
                .is_some_and(|name| name.isEqualToString(&layer_name))
        })
    }) {
        layer.setHidden(hidden);
    }
}

fn find_or_create_bubble_layer(root_layer: &CALayer) -> objc2::rc::Retained<CALayer> {
    let layer_name = NSString::from_str(BUBBLE_LAYER_NAME);
    if let Some(layer) = unsafe { root_layer.sublayers() }.and_then(|layers| {
        layers.into_iter().find(|layer| {
            layer
                .name()
                .is_some_and(|name| name.isEqualToString(&layer_name))
        })
    }) {
        return layer;
    }

    let layer = CALayer::layer();
    layer.setName(Some(&layer_name));
    root_layer.addSublayer(&layer);
    layer
}

fn native_window(window: &Window) -> LocalResult<&'static NSWindow> {
    let ns_window = window.ns_window().map_err(|err| err.to_string())?;
    if ns_window.is_null() {
        return Err("macOS 桌宠窗口句柄为空。".into());
    }

    Ok(unsafe { &*(ns_window.cast::<NSWindow>()) })
}
