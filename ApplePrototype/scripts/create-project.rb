require 'xcodeproj'
root = File.expand_path('..', __dir__)
project = Xcodeproj::Project.new(File.join(root, 'SolidcraftForIPad.xcodeproj'))
target = project.new_target(:application, 'SolidcraftForIPad', :ios, '17.0')
group = project.main_group.new_group('SolidcraftForIPad', 'SolidcraftForIPad')
Dir[File.join(root, 'SolidcraftForIPad', '*.swift')].sort.each { |path| target.source_build_phase.add_file_reference(group.new_file(File.basename(path))) }
['input.js', 'bridge.js', 'Assets.xcassets'].each { |path| target.resources_build_phase.add_file_reference(group.new_file(path)) }
web = group.new_file('Web'); web.last_known_file_type = 'folder'; target.resources_build_phase.add_file_reference(web)
target.build_configurations.each do |config|
 s=config.build_settings
 s['PRODUCT_BUNDLE_IDENTIFIER']='com.cwallyman.SolidcraftForIPad'
 s['DEVELOPMENT_TEAM']='D58RZM4K8N'
 s['CODE_SIGN_STYLE']='Automatic'
 s['SWIFT_VERSION']='5.0'
 s['GENERATE_INFOPLIST_FILE']='YES'
 s['INFOPLIST_FILE']='SolidcraftForIPad/Info.plist'
 s['INFOPLIST_KEY_CFBundleDisplayName']='Solidcraft for iPad'
 s['INFOPLIST_KEY_UILaunchScreen_Generation']='YES'
 s['TARGETED_DEVICE_FAMILY']='2'
 s['SUPPORTS_MACCATALYST']='YES'
 s['DERIVE_MACCATALYST_PRODUCT_BUNDLE_IDENTIFIER']='YES'
 s['CURRENT_PROJECT_VERSION']='1'
 s['MARKETING_VERSION']='0.1.0'
 s['ASSETCATALOG_COMPILER_APPICON_NAME']='AppIcon'
 s['ENABLE_USER_SCRIPT_SANDBOXING']='YES'
end
ui = project.new_target(:ui_test_bundle, 'SolidcraftForIPadUITests', :ios, '17.0')
ui.add_dependency(target)
ug = project.main_group.new_group('SolidcraftForIPadUITests','SolidcraftForIPadUITests')
Dir[File.join(root,'SolidcraftForIPadUITests','*.swift')].each { |path| ui.source_build_phase.add_file_reference(ug.new_file(File.basename(path))) }
ui.build_configurations.each do |config|
 s=config.build_settings
 s['PRODUCT_BUNDLE_IDENTIFIER']='com.cwallyman.SolidcraftForIPadUITests'
 s['DEVELOPMENT_TEAM']='D58RZM4K8N'
 s['CODE_SIGN_STYLE']='Automatic'
 s['SWIFT_VERSION']='5.0'
 s['GENERATE_INFOPLIST_FILE']='YES'
 s['TEST_TARGET_NAME']='SolidcraftForIPad'
 s['TARGETED_DEVICE_FAMILY']='2'
end
project.save
scheme=Xcodeproj::XCScheme.new
scheme.add_build_target(target)
scheme.add_test_target(ui)
scheme.set_launch_target(target)
scheme.save_as(project.path, 'SolidcraftForIPad', true)
