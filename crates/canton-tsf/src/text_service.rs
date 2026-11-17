use crate::display_attr::{CantonDisplayAttributeInfo, CantonEnumDisplayAttributeInfo};
use crate::guids::{GUID_CANTON_DISPLAY_ATTRIBUTE, GUID_CANTON_PROFILE};
use crate::langbar::CantonLangBarButton;
use canton_core::{Engine, EngineAction, KeyEvent};
use canton_ui::CandidateWindow;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use windows::core::{implement, ComObject, GUID, HRESULT, Interface, VARIANT};
use windows::Win32::Foundation::{BOOL, E_FAIL, E_INVALIDARG, LPARAM, RECT, S_OK, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardState, ToUnicode, VK_BACK, VK_CONTROL, VK_DOWN, VK_ESCAPE, VK_LEFT, VK_LSHIFT,
    VK_LWIN, VK_MENU, VK_NEXT, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_RSHIFT, VK_RWIN, VK_SHIFT,
    VK_SPACE, VK_UP,
};
use windows::Win32::UI::TextServices::{
    IEnumTfDisplayAttributeInfo, ITfCompartmentEventSink, ITfCompartmentEventSink_Impl,
    ITfCompartmentMgr, ITfComposition, ITfCompositionSink, ITfCompositionSink_Impl, ITfContext,
    ITfContextComposition, ITfDisplayAttributeInfo, ITfDisplayAttributeProvider,
    ITfDisplayAttributeProvider_Impl, ITfDocumentMgr, ITfEditSession, ITfEditSession_Impl,
    ITfKeyEventSink, ITfKeyEventSink_Impl, ITfKeystrokeMgr, ITfLangBarItemMgr, ITfRange,
    ITfSource, ITfTextInputProcessor, ITfTextInputProcessorEx, ITfTextInputProcessorEx_Impl,
    ITfTextInputProcessor_Impl, ITfThreadMgr, ITfThreadMgrEventSink, ITfThreadMgrEventSink_Impl,
    GUID_COMPARTMENT_KEYBOARD_OPENCLOSE, GUID_PROP_ATTRIBUTE, TF_ES_ASYNCDONTCARE,
    TF_ES_READWRITE, TF_INVALID_COOKIE,
};

struct ServiceState {
    thread_mgr: Option<ITfThreadMgr>,
    client_id: u32,
    thread_mgr_sink_cookie: u32,
    open_close_cookie: u32,
    display_attr_atom: u32,
    engine: Engine,
    chinese_mode: Arc<AtomicBool>,
    composition: Option<ITfComposition>,
    candidate_wnd: Option<CandidateWindow>,
    lang_bar_btn: Option<ComObject<CantonLangBarButton>>,
    shift_tap_pending: bool,
}

impl ServiceState {
    fn new() -> Self {
        Self {
            thread_mgr: None,
            client_id: 0,
            thread_mgr_sink_cookie: TF_INVALID_COOKIE,
            open_close_cookie: TF_INVALID_COOKIE,
            display_attr_atom: 0,
            engine: Engine::default(),
            chinese_mode: Arc::new(AtomicBool::new(true)),
            composition: None,
            candidate_wnd: None,
            lang_bar_btn: None,
            shift_tap_pending: false,
        }
    }

    fn ensure_candidate_window(&mut self) -> Option<&mut CandidateWindow> {
        if self.candidate_wnd.is_none() {
            if let Ok(wnd) = CandidateWindow::new() {
                self.candidate_wnd = Some(wnd);
            }
        }
        self.candidate_wnd.as_mut()
    }

    fn hide_candidate_window(&mut self) {
        if let Some(wnd) = self.candidate_wnd.as_mut() {
            wnd.hide();
        }
    }
}

#[implement(
    ITfTextInputProcessorEx,
    ITfTextInputProcessor,
    ITfThreadMgrEventSink,
    ITfKeyEventSink,
    ITfCompositionSink,
    ITfDisplayAttributeProvider,
    ITfCompartmentEventSink
)]
pub struct CantonTextService {
    state: Rc<RefCell<ServiceState>>,
}

impl CantonTextService {
    pub fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(ServiceState::new())),
        }
    }

    fn activate_internal(&self, ptim: Option<&ITfThreadMgr>, tid: u32) -> windows::core::Result<()> {
        let Some(ptim) = ptim else {
            return Err(E_INVALIDARG.into());
        };

        {
            let mut st = self.state.borrow_mut();
            st.thread_mgr = Some(ptim.clone());
            st.client_id = tid;
        }

        // Register ITfThreadMgrEventSink
        if let Ok(source) = ptim.cast::<ITfSource>() {
            let sink: ITfThreadMgrEventSink = unsafe { self.cast()? };
            if let Ok(cookie) = unsafe { source.AdviseSink(&ITfThreadMgrEventSink::IID, &sink) } {
                self.state.borrow_mut().thread_mgr_sink_cookie = cookie;
            }
        }

        // Register ITfKeyEventSink
        if let Ok(keystroke_mgr) = ptim.cast::<ITfKeystrokeMgr>() {
            let key_sink: ITfKeyEventSink = unsafe { self.cast()? };
            unsafe {
                keystroke_mgr.AdviseKeyEventSink(tid, &key_sink, true)?;
            }
        }

        // Register DisplayAttribute atom for GUID_CANTON_DISPLAY_ATTRIBUTE
        if let Ok(cat_mgr) = unsafe {
            windows::Win32::System::Com::CoCreateInstance::<
                _,
                windows::Win32::UI::TextServices::ITfCategoryMgr,
            >(
                &windows::Win32::UI::TextServices::CLSID_TF_CategoryMgr,
                None,
                windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
            )
        } {
            if let Ok(atom) = unsafe { cat_mgr.RegisterGUID(&GUID_CANTON_DISPLAY_ATTRIBUTE) } {
                self.state.borrow_mut().display_attr_atom = atom;
            }
        }

        // Sync with GUID_COMPARTMENT_KEYBOARD_OPENCLOSE
        self.setup_open_close_compartment(ptim, tid);

        // Add LangBar button
        self.setup_lang_bar(ptim);

        Ok(())
    }

    fn setup_open_close_compartment(&self, ptim: &ITfThreadMgr, tid: u32) {
        let Ok(comp_mgr) = ptim.cast::<ITfCompartmentMgr>() else {
            return;
        };
        let Ok(comp) = (unsafe { comp_mgr.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE) })
        else {
            return;
        };
        // Initialize to open (Chinese mode = true)
        unsafe {
            let var = VARIANT::from(1i32);
            let _ = comp.SetValue(tid, &var);
        }
        if let Ok(source) = comp.cast::<ITfSource>() {
            if let Ok(sink) = unsafe { self.cast::<ITfCompartmentEventSink>() } {
                if let Ok(cookie) =
                    unsafe { source.AdviseSink(&ITfCompartmentEventSink::IID, &sink) }
                {
                    self.state.borrow_mut().open_close_cookie = cookie;
                }
            }
        }
    }

    fn setup_lang_bar(&self, ptim: &ITfThreadMgr) {
        let Ok(lb_mgr) = (unsafe {
            windows::Win32::System::Com::CoCreateInstance::<_, ITfLangBarItemMgr>(
                &windows::Win32::UI::TextServices::CLSID_TF_LangBarItemMgr,
                None,
                windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
            )
        }) else {
            return;
        };

        let chinese_flag = self.state.borrow().chinese_mode.clone();
        let tm_weak = ptim.clone();
        let client_id = self.state.borrow().client_id;
        let btn = ComObject::new(CantonLangBarButton::new(
            chinese_flag.clone(),
            Box::new(move || {
                let next = !chinese_flag.load(Ordering::Relaxed);
                chinese_flag.store(next, Ordering::Relaxed);
                if let Ok(comp_mgr) = tm_weak.cast::<ITfCompartmentMgr>() {
                    if let Ok(comp) = unsafe {
                        comp_mgr.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE)
                    } {
                        unsafe {
                            let var = VARIANT::from(if next { 1i32 } else { 0i32 });
                            let _ = comp.SetValue(client_id, &var);
                        }
                    }
                }
            }),
        ));

        let item: windows::Win32::UI::TextServices::ITfLangBarItem = btn.to_interface();
        if unsafe { lb_mgr.AddItem(&item) }.is_ok() {
            self.state.borrow_mut().lang_bar_btn = Some(btn);
        }
    }

    fn toggle_open_close(&self) {
        let (tm_opt, tid, next) = {
            let mut st = self.state.borrow_mut();
            let next = st.engine.toggle_chinese_mode();
            st.chinese_mode.store(next, Ordering::Relaxed);
            st.hide_candidate_window();
            (st.thread_mgr.clone(), st.client_id, next)
        };
        if let Some(tm) = tm_opt {
            if let Ok(comp_mgr) = tm.cast::<ITfCompartmentMgr>() {
                if let Ok(comp) =
                    unsafe { comp_mgr.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE) }
                {
                    unsafe {
                        let var = VARIANT::from(if next { 1i32 } else { 0i32 });
                        let _ = comp.SetValue(tid, &var);
                    }
                }
            }
        }
        if let Some(btn) = self.state.borrow().lang_bar_btn.as_ref() {
            btn.notify_update();
        }
    }
}

impl ITfTextInputProcessor_Impl for CantonTextService_Impl {
    fn Activate(&self, ptim: Option<&ITfThreadMgr>, tid: u32) -> windows::core::Result<()> {
        self.activate_internal(ptim, tid)
    }

    fn Deactivate(&self) -> windows::core::Result<()> {
        let mut st = self.state.borrow_mut();
        st.hide_candidate_window();
        st.candidate_wnd = None;
        st.composition = None;
        st.engine.reset();

        if let Some(tm) = st.thread_mgr.take() {
            if let Ok(keystroke_mgr) = tm.cast::<ITfKeystrokeMgr>() {
                unsafe {
                    let _ = keystroke_mgr.UnadviseKeyEventSink(st.client_id);
                }
            }
            if st.thread_mgr_sink_cookie != TF_INVALID_COOKIE {
                if let Ok(source) = tm.cast::<ITfSource>() {
                    unsafe {
                        let _ = source.UnadviseSink(st.thread_mgr_sink_cookie);
                    }
                }
                st.thread_mgr_sink_cookie = TF_INVALID_COOKIE;
            }
            if st.open_close_cookie != TF_INVALID_COOKIE {
                if let Ok(comp_mgr) = tm.cast::<ITfCompartmentMgr>() {
                    if let Ok(comp) =
                        unsafe { comp_mgr.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE) }
                    {
                        if let Ok(source) = comp.cast::<ITfSource>() {
                            unsafe {
                                let _ = source.UnadviseSink(st.open_close_cookie);
                            }
                        }
                    }
                }
                st.open_close_cookie = TF_INVALID_COOKIE;
            }
            if let Some(btn) = st.lang_bar_btn.take() {
                if let Ok(lb_mgr) = unsafe {
                    windows::Win32::System::Com::CoCreateInstance::<_, ITfLangBarItemMgr>(
                        &windows::Win32::UI::TextServices::CLSID_TF_LangBarItemMgr,
                        None,
                        windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
                    )
                } {
                    let item: windows::Win32::UI::TextServices::ITfLangBarItem = btn.to_interface();
                    unsafe {
                        let _ = lb_mgr.RemoveItem(&item);
                    }
                }
            }
        }
        st.client_id = 0;
        Ok(())
    }
}

impl ITfTextInputProcessorEx_Impl for CantonTextService_Impl {
    fn ActivateEx(
        &self,
        ptim: Option<&ITfThreadMgr>,
        tid: u32,
        _dwflags: u32,
    ) -> windows::core::Result<()> {
        self.activate_internal(ptim, tid)
    }
}

impl ITfThreadMgrEventSink_Impl for CantonTextService_Impl {
    fn OnInitDocumentMgr(&self, _pdim: Option<&ITfDocumentMgr>) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnUninitDocumentMgr(&self, _pdim: Option<&ITfDocumentMgr>) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnSetFocus(
        &self,
        _pdimfocus: Option<&ITfDocumentMgr>,
        _pdimprevfocus: Option<&ITfDocumentMgr>,
    ) -> windows::core::Result<()> {
        let mut st = self.state.borrow_mut();
        if st.engine.is_composing() {
            st.engine.reset();
            st.composition = None;
            st.hide_candidate_window();
        }
        Ok(())
    }

    fn OnPushContext(&self, _pic: Option<&ITfContext>) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnPopContext(&self, _pic: Option<&ITfContext>) -> windows::core::Result<()> {
        Ok(())
    }
}

impl ITfKeyEventSink_Impl for CantonTextService_Impl {
    fn OnSetFocus(&self, _fforeground: BOOL) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnTestKeyDown(
        &self,
        _pic: Option<&ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> windows::core::Result<BOOL> {
        let vk = wparam.0 as u16;
        let mut st = self.state.borrow_mut();

        if vk == VK_SHIFT.0 || vk == VK_LSHIFT.0 || vk == VK_RSHIFT.0 {
            st.shift_tap_pending = true;
            return Ok(BOOL(0));
        }
        st.shift_tap_pending = false;

        if is_modifier_chord_held() {
            return Ok(BOOL(0));
        }

        if !st.engine.is_chinese_mode() {
            return Ok(BOOL(0));
        }

        let Some(ev) = translate_vk(vk) else {
            return Ok(BOOL(0));
        };

        let eat = will_consume_key(&st.engine, ev);
        Ok(BOOL(if eat { 1 } else { 0 }))
    }

    fn OnTestKeyUp(
        &self,
        _pic: Option<&ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> windows::core::Result<BOOL> {
        let vk = wparam.0 as u16;
        if vk == VK_SHIFT.0 || vk == VK_LSHIFT.0 || vk == VK_RSHIFT.0 {
            let pending = {
                let mut st = self.state.borrow_mut();
                let p = st.shift_tap_pending;
                st.shift_tap_pending = false;
                p
            };
            if pending {
                self.toggle_open_close();
            }
        }
        Ok(BOOL(0))
    }

    fn OnKeyDown(
        &self,
        pic: Option<&ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> windows::core::Result<BOOL> {
        let Some(context) = pic else {
            return Ok(BOOL(0));
        };
        let vk = wparam.0 as u16;
        if is_modifier_chord_held() {
            return Ok(BOOL(0));
        }

        let Some(ev) = translate_vk(vk) else {
            return Ok(BOOL(0));
        };

        let action = {
            let mut st = self.state.borrow_mut();
            if !st.engine.is_chinese_mode() {
                return Ok(BOOL(0));
            }
            st.engine.handle_key(ev)
        };

        match action {
            EngineAction::PassThrough => Ok(BOOL(0)),
            EngineAction::Reject => Ok(BOOL(1)),
            other => {
                let client_id = self.state.borrow().client_id;
                let sink: ITfCompositionSink = unsafe { self.cast()? };
                let session: ITfEditSession = EditSession::new(
                    self.state.clone(),
                    context.clone(),
                    sink,
                    other,
                )
                .into();
                let _ = unsafe {
                    context.RequestEditSession(
                        client_id,
                        &session,
                        TF_ES_ASYNCDONTCARE | TF_ES_READWRITE,
                    )
                };
                Ok(BOOL(1))
            }
        }
    }

    fn OnKeyUp(
        &self,
        _pic: Option<&ITfContext>,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> windows::core::Result<BOOL> {
        Ok(BOOL(0))
    }

    fn OnPreservedKey(
        &self,
        _pic: Option<&ITfContext>,
        _rguid: *const GUID,
    ) -> windows::core::Result<BOOL> {
        Ok(BOOL(0))
    }
}

impl ITfCompositionSink_Impl for CantonTextService_Impl {
    fn OnCompositionTerminated(
        &self,
        _ecwrite: u32,
        _pcomposition: Option<&ITfComposition>,
    ) -> windows::core::Result<()> {
        let mut st = self.state.borrow_mut();
        st.composition = None;
        st.engine.reset();
        st.hide_candidate_window();
        Ok(())
    }
}

impl ITfDisplayAttributeProvider_Impl for CantonTextService_Impl {
    fn EnumDisplayAttributeInfo(&self) -> windows::core::Result<IEnumTfDisplayAttributeInfo> {
        Ok(CantonEnumDisplayAttributeInfo::new().into())
    }

    fn GetDisplayAttributeInfo(
        &self,
        guid: *const GUID,
    ) -> windows::core::Result<ITfDisplayAttributeInfo> {
        if guid.is_null() {
            return Err(E_INVALIDARG.into());
        }
        unsafe {
            if *guid == GUID_CANTON_DISPLAY_ATTRIBUTE {
                Ok(CantonDisplayAttributeInfo::new().into())
            } else {
                Err(E_FAIL.into())
            }
        }
    }
}

impl ITfCompartmentEventSink_Impl for CantonTextService_Impl {
    fn OnChange(&self, rguid: *const GUID) -> windows::core::Result<()> {
        if rguid.is_null() {
            return Ok(());
        }
        unsafe {
            if *rguid == GUID_COMPARTMENT_KEYBOARD_OPENCLOSE {
                let tm_opt = self.state.borrow().thread_mgr.clone();
                if let Some(tm) = tm_opt {
                    if let Ok(comp_mgr) = tm.cast::<ITfCompartmentMgr>() {
                        if let Ok(comp) =
                            comp_mgr.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE)
                        {
                            if let Ok(var) = comp.GetValue() {
                                let open = i32::try_from(&var).unwrap_or(1) != 0;
                                let mut st = self.state.borrow_mut();
                                st.engine.set_chinese_mode(open);
                                st.chinese_mode.store(open, Ordering::Relaxed);
                                if !open {
                                    st.hide_candidate_window();
                                }
                                if let Some(btn) = st.lang_bar_btn.as_ref() {
                                    btn.notify_update();
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[implement(ITfEditSession)]
struct EditSession {
    state: Rc<RefCell<ServiceState>>,
    context: ITfContext,
    comp_sink: ITfCompositionSink,
    action: EngineAction,
}

impl EditSession {
    fn new(
        state: Rc<RefCell<ServiceState>>,
        context: ITfContext,
        comp_sink: ITfCompositionSink,
        action: EngineAction,
    ) -> Self {
        Self {
            state,
            context,
            comp_sink,
            action,
        }
    }
}

impl ITfEditSession_Impl for EditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> windows::core::Result<()> {
        unsafe {
            match &self.action {
                EngineAction::UpdateUi(page) => {
                    let range = ensure_composition(
                        ec,
                        &self.context,
                        &self.comp_sink,
                        &mut self.state.borrow_mut(),
                    )?;
                    let w: Vec<u16> = page.preedit.encode_utf16().collect();
                    range.SetText(ec, 0, &w)?;
                    apply_display_attr(ec, &self.context, &range, self.state.borrow().display_attr_atom);
                    position_and_show_ui(ec, &self.context, &range, page, &mut self.state.borrow_mut());
                }
                EngineAction::Commit(text) => {
                    let mut st = self.state.borrow_mut();
                    if let Some(comp) = st.composition.take() {
                        if let Ok(range) = comp.GetRange() {
                            let w: Vec<u16> = text.encode_utf16().collect();
                            let _ = range.SetText(ec, 0, &w);
                            clear_display_attr(ec, &self.context, &range);
                            let _ = range.Collapse(ec, windows::Win32::UI::TextServices::TF_ANCHOR_END);
                        }
                        let _ = comp.EndComposition(ec);
                    } else {
                        insert_text_at_selection(ec, &self.context, text);
                    }
                    st.hide_candidate_window();
                }
                EngineAction::CommitAndUpdate {
                    committed,
                    next_page,
                } => {
                    let mut st = self.state.borrow_mut();
                    if let Some(comp) = st.composition.take() {
                        if let Ok(range) = comp.GetRange() {
                            let w: Vec<u16> = committed.encode_utf16().collect();
                            let _ = range.SetText(ec, 0, &w);
                            clear_display_attr(ec, &self.context, &range);
                            let _ = range.Collapse(ec, windows::Win32::UI::TextServices::TF_ANCHOR_END);
                        }
                        let _ = comp.EndComposition(ec);
                    } else {
                        insert_text_at_selection(ec, &self.context, committed);
                    }
                    let range = ensure_composition(ec, &self.context, &self.comp_sink, &mut st)?;
                    let w: Vec<u16> = next_page.preedit.encode_utf16().collect();
                    range.SetText(ec, 0, &w)?;
                    apply_display_attr(ec, &self.context, &range, st.display_attr_atom);
                    position_and_show_ui(ec, &self.context, &range, next_page, &mut st);
                }
                EngineAction::Cancel => {
                    let mut st = self.state.borrow_mut();
                    if let Some(comp) = st.composition.take() {
                        if let Ok(range) = comp.GetRange() {
                            let _ = range.SetText(ec, 0, &[]);
                            clear_display_attr(ec, &self.context, &range);
                        }
                        let _ = comp.EndComposition(ec);
                    }
                    st.hide_candidate_window();
                }
                EngineAction::PassThrough | EngineAction::Reject => {}
            }
        }
        Ok(())
    }
}

unsafe fn ensure_composition(
    ec: u32,
    context: &ITfContext,
    comp_sink: &ITfCompositionSink,
    st: &mut ServiceState,
) -> windows::core::Result<ITfRange> {
    if let Some(comp) = st.composition.as_ref() {
        return comp.GetRange();
    }
    let insert_sel: windows::Win32::UI::TextServices::ITfInsertAtSelection = context.cast()?;
    let range = insert_sel.InsertTextAtSelection(ec, windows::Win32::UI::TextServices::TF_IAS_QUERYONLY, &[])?;
    let ctx_comp: ITfContextComposition = context.cast()?;
    let comp = ctx_comp.StartComposition(ec, &range, comp_sink)?;
    let comp_range = comp.GetRange()?;
    st.composition = Some(comp);
    Ok(comp_range)
}

unsafe fn insert_text_at_selection(ec: u32, context: &ITfContext, text: &str) {
    if let Ok(insert_sel) = context.cast::<windows::Win32::UI::TextServices::ITfInsertAtSelection>() {
        let w: Vec<u16> = text.encode_utf16().collect();
        let _ = insert_sel.InsertTextAtSelection(
            ec,
            windows::Win32::UI::TextServices::INSERT_TEXT_AT_SELECTION_FLAGS(0),
            &w,
        );
    }
}

unsafe fn apply_display_attr(ec: u32, context: &ITfContext, range: &ITfRange, atom: u32) {
    if atom == 0 {
        return;
    }
    if let Ok(prop) = context.GetProperty(&GUID_PROP_ATTRIBUTE) {
        let var = VARIANT::from(atom as i32);
        let _ = prop.SetValue(ec, range, &var);
    }
}

unsafe fn clear_display_attr(ec: u32, context: &ITfContext, range: &ITfRange) {
    if let Ok(prop) = context.GetProperty(&GUID_PROP_ATTRIBUTE) {
        let _ = prop.Clear(ec, range);
    }
}

unsafe fn position_and_show_ui(
    ec: u32,
    context: &ITfContext,
    range: &ITfRange,
    page: &canton_core::CandidatePage,
    st: &mut ServiceState,
) {
    let mut rc = RECT::default();
    let mut clipped = BOOL(0);
    if let Ok(view) = context.GetActiveView() {
        let _ = view.GetTextExt(ec, range, &mut rc, &mut clipped);
    }
    let height = (rc.bottom - rc.top).max(20);
    if let Some(wnd) = st.ensure_candidate_window() {
        wnd.show_page(page, rc.left, rc.top, height);
    }
}

fn is_modifier_chord_held() -> bool {
    unsafe {
        let mut ks = [0u8; 256];
        if GetKeyboardState(&mut ks).is_err() {
            return false;
        }
        (ks[VK_CONTROL.0 as usize] & 0x80) != 0
            || (ks[VK_MENU.0 as usize] & 0x80) != 0
            || (ks[VK_LWIN.0 as usize] & 0x80) != 0
            || (ks[VK_RWIN.0 as usize] & 0x80) != 0
    }
}

fn translate_vk(vk: u16) -> Option<KeyEvent> {
    match vk {
        x if x == VK_BACK.0 => Some(KeyEvent::Backspace),
        x if x == VK_SPACE.0 => Some(KeyEvent::Space),
        x if x == VK_RETURN.0 => Some(KeyEvent::Enter),
        x if x == VK_ESCAPE.0 => Some(KeyEvent::Escape),
        x if x == VK_NEXT.0 => Some(KeyEvent::PageDown),
        x if x == VK_PRIOR.0 => Some(KeyEvent::PageUp),
        x if x == VK_DOWN.0 => Some(KeyEvent::Down),
        x if x == VK_UP.0 => Some(KeyEvent::Up),
        x if x == VK_LEFT.0 => Some(KeyEvent::Left),
        x if x == VK_RIGHT.0 => Some(KeyEvent::Right),
        _ => unsafe {
            let mut ks = [0u8; 256];
            if GetKeyboardState(&mut ks).is_err() {
                return None;
            }
            // Do not read CapsLock-inverted ASCII if we want predictable mapping; ToUnicode handles Shift
            let mut buf = [0u16; 4];
            let count = ToUnicode(vk as u32, 0, Some(&ks), &mut buf, 0);
            if count == 1 {
                char::from_u32(buf[0] as u32).map(KeyEvent::Char)
            } else {
                None
            }
        },
    }
}

fn will_consume_key(engine: &Engine, ev: KeyEvent) -> bool {
    if !engine.is_chinese_mode() {
        return false;
    }
    if engine.is_composing() {
        return true;
    }
    match ev {
        KeyEvent::Char(ch) if ch.is_ascii_lowercase() => true,
        KeyEvent::Char(ch) if !ch.is_ascii_digit() && !ch.is_ascii_uppercase() => {
            canton_core::Lexicon::lookup_count(&ch.to_string()) > 0
        }
        _ => false,
    }
}

#[allow(dead_code)]
fn _unused_s_ok() -> HRESULT {
    let _ = GUID_CANTON_PROFILE;
    S_OK
}
