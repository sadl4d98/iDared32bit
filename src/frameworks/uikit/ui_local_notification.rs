/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UILocalNotification`.

use crate::frameworks::foundation::{NSInteger, NSUInteger};
use crate::objc::{id, objc_classes, todo_objc_setter, ClassExports};

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UILocalNotification: NSObject

- (())setFireDate:(id)date { // NSDate *
    todo_objc_setter!(this, date);
}
- (())setTimeZone:(id)time_zone { // NSTimeZone *
    todo_objc_setter!(this, time_zone);
}
- (())setAlertBody:(id)body { // NSString *
    todo_objc_setter!(this, body);
}
- (())setAlertAction:(id)action { // NSString *
    todo_objc_setter!(this, action);
}
- (())setSoundName:(id)name { // NSString *
    todo_objc_setter!(this, name);
}
- (())setApplicationIconBadgeNumber:(NSInteger)badge_number {
    todo_objc_setter!(this, badge_number);
}
- (())setRepeatInterval:(NSUInteger)interval { // NSCalendarUnit
    todo_objc_setter!(this, interval);
}
- (())setUserInfo:(id)user_info { // NSDictionary *
    todo_objc_setter!(this, user_info);
}
- (())setHasAction:(bool)has_action {
    todo_objc_setter!(this, has_action);
}
- (())setAlertLaunchImage:(id)image_name { // NSString *
    todo_objc_setter!(this, image_name);
}

@end

};
