use crate::app::ScrollDirection;
use crate::kernel::cmd::{Command, ModuleCommand};
use crate::style::{Style, StyledText};
use crate::util;
use bytesize::ByteSize;
use tui::widgets::Text;

/* Loadable kernel modules */
pub struct KernelModules<'a> {
	pub default_list: Vec<Vec<String>>,
	pub list: Vec<Vec<String>>,
	pub current_name: String,
	pub current_info: StyledText<'a>,
	pub command: ModuleCommand,
	pub index: usize,
	pub info_scroll_offset: usize,
}

impl KernelModules<'_> {
	/**
	 * Create a new kernel modules instance.
	 *
	 * @param  args
	 * @return KernelModules
	 */
	pub fn new(args: &clap::ArgMatches) -> Self {
		let mut module_list: Vec<Vec<String>> = Vec::new();
		/* Set the command for reading kernel modules and execute it. */
		let mut module_read_cmd = String::from("cat /proc/modules");
		if let Some(matches) = args.subcommand_matches("sort") {
			if matches.is_present("size") {
				module_read_cmd += " | sort -n -r -t ' ' -k2";
			} else {
				module_read_cmd += " | sort -t ' ' -k1";
			}
		}
		let modules_content = util::exec_cmd("sh", &["-c", &module_read_cmd])
			.expect("failed to read /proc/modules");
		/* Parse content for module name, size and related information. */
		for line in modules_content.lines() {
			let columns: Vec<&str> = line.split_whitespace().collect();
			let mut module_name = columns[0].to_string();
			if columns.len() >= 7 {
				module_name = format!("{} {}", module_name, columns[6]);
			}
			let mut used_modules = format!("{} {}", columns[2], columns[3]);
			if used_modules.ends_with(',') {
				used_modules.pop();
			}
			let module_size =
				ByteSize::b(columns[1].to_string().parse().unwrap()).to_string();
			module_list.push(vec![module_name, module_size, used_modules]);
		}
		/* Reverse the kernel modules if the argument is provided. */
		if args.is_present("reverse") {
			module_list.reverse();
		}
		/* Scroll modules to top and return. */
		let mut kernel_modules = Self {
			default_list: module_list.clone(),
			list: module_list,
			current_name: String::new(),
			current_info: StyledText::default(),
			command: ModuleCommand::None,
			index: 0,
			info_scroll_offset: 0,
		};
		kernel_modules.scroll_list(ScrollDirection::Top);
		kernel_modules
	}

	/**
	 * Get the current command using current module name.
	 *
	 * @return Command
	 */
	pub fn get_current_command(&self) -> Command {
		self.command.get(&self.current_name)
	}

	/**
	 * Set the current module command and show confirmation message.
	 *
	 * @param module_command
	 */
	pub fn set_current_command(&mut self, module_command: ModuleCommand) {
		self.command = module_command;
		self.current_info.set_styled_text(
			vec![
				Text::styled(
					"\nExecute the following command? [y/N]:\n\n",
					Style::default().unselected_style,
				),
				Text::styled(
					self.get_current_command().cmd,
					Style::default().selected_style,
				),
				Text::styled(
					format!("\n\n{}", self.get_current_command().desc),
					Style::default().unselected_style,
				),
			],
			5,
		);
		self.info_scroll_offset = 0;
	}

	/**
	 * Execute the current module command.
	 *
	 * @return command_executed
	 */
	pub fn exec_current_command(&mut self) -> bool {
		let mut command_executed = false;
		if !self.command.is_none() {
			match util::exec_cmd("sh", &["-c", &self.get_current_command().cmd]) {
				Ok(_) => command_executed = true,
				Err(e) => self.current_info.set_styled_text(
					vec![
						Text::styled(
							"\nFailed to execute command: ",
							Style::default().unselected_style,
						),
						Text::styled(
							format!("'{}'\n\n{}", self.get_current_command().cmd, e),
							Style::default().selected_style,
						),
					],
					3,
				),
			}
			self.command = ModuleCommand::None;
		}
		command_executed
	}

	/**
	 * Scroll to the position of used module at given index.
	 *
	 * @param index
	 */
	pub fn show_used_module_info(&mut self, index: usize) {
		let used_module =
			(*self.list[self.index][2].split(' ').collect::<Vec<&str>>()[1]
				.split(',')
				.collect::<Vec<&str>>()
				.get(index)
				.unwrap_or(&"-"))
			.to_string();
		if used_module != "-" {
			self.index = self
				.list
				.iter()
				.position(|module| module[0] == used_module)
				.unwrap_or(self.index)
				- 1;
			self.scroll_list(ScrollDirection::Down);
		}
	}

	/**
	 * Scroll module list up/down and select module.
	 *
	 * @param direction
	 */
	pub fn scroll_list(&mut self, direction: ScrollDirection) {
		self.info_scroll_offset = 0;
		if self.list.is_empty() {
			self.index = 0;
		} else {
			/* Scroll module list. */
			match direction {
				ScrollDirection::Up => self.previous_module(),
				ScrollDirection::Down => self.next_module(),
				ScrollDirection::Top => self.index = 0,
				ScrollDirection::Bottom => self.index = self.list.len() - 1,
			}
			/* Set current module name. */
			self.current_name = self.list[self.index][0]
				.split_whitespace()
				.next()
				.unwrap()
				.to_string();
			/* Execute 'modinfo' and add style to its output. */
			self.current_info.stylize_data(
				Box::leak(
					util::exec_cmd("modinfo", &[&self.current_name])
						.unwrap_or_else(|_| {
							String::from("failed to retrieve module information")
						})
						.replace("signature: ", "signature: \n")
						.into_boxed_str(),
				),
				':',
			);
			/* Clear the current command. */
			if !self.command.is_none() {
				self.command = ModuleCommand::None;
			}
		}
	}

	/**
	 * Select the next module.
	 */
	pub fn next_module(&mut self) {
		self.index += 1;
		if self.index > self.list.len() - 1 {
			self.index = 0;
		}
	}

	/**
	 * Select the previous module.
	 */
	pub fn previous_module(&mut self) {
		if self.index > 0 {
			self.index -= 1;
		} else {
			self.index = self.list.len() - 1;
		}
	}

	/**
	 * Scroll the module information text up/down.
	 *
	 * @param direction
	 */
	pub fn scroll_mod_info(&mut self, direction: ScrollDirection, smooth_scroll: bool) {
		let scroll_amount = if smooth_scroll { 1 } else { 2 };
		match direction {
			ScrollDirection::Up => {
				if self.info_scroll_offset > scroll_amount - 1 {
					self.info_scroll_offset -= scroll_amount;
				}
			}
			ScrollDirection::Down => {
				if self.current_info.lines() > 0 {
					self.info_scroll_offset += scroll_amount;
					self.info_scroll_offset %=
						((self.current_info.lines() as u16) * 2) as usize;
				}
			}
			_ => {}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use clap::App;
	#[test]
	fn test_kernel_modules() {
		let matches = App::new("test").get_matches();
		let kernel_modules = KernelModules::new(&matches);
		assert_eq!(0, kernel_modules.index);
		assert_ne!(0, kernel_modules.default_list.len());
		assert_ne!(0, kernel_modules.current_name.len());
		assert_ne!(0, kernel_modules.current_info.lines());
	}
}
