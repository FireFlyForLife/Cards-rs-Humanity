/**
 * \file `CrsH-Frontend.js`
 * 
 * \brief this file contains the logic for visualising the Cards-rs-Humanity game.
 * \dependson `CrsH-Gameplay.js` `jquery-3.4.0.js` `signals.js` `jquery-form.js`
 */

$(document).ready(function() {
    $('#loginForm').ajaxForm({
		success: function() {
			alert("Thank you for your login!");
		},
		error: function(request, status, error) {
			alert("ERROR on login. Some info: " + request.responseText + " + " + error + " + " + status);
		}
	});
	$('#registerForm').ajaxForm({
		success: function() {
			alert("Thank you for your registering!");
		},
		error: function(request, status, error) {
			alert("ERROR onregister. Some info: " + request.responseText + " + " + error + " + " + status);
		}
	});
});
