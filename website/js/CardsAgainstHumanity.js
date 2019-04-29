var handOfCardsContainer = null;
var hasSubmittedCard = false;
var submitButton = null;

function deselectCards() {
	var whiteCards = document.getElementsByClassName("whiteCard");
	for (var i = 0; i < whiteCards.length; i++) {
		whiteCards[i].classList.remove("selectedCard");
	}
}

function selectCard(whiteCard) {
	whiteCard.classList.add("selectedCard");
}

function addWhiteCard(text) {
	/* Create a card like this:
		<div class="whiteCard">The text of the card</div>
	*/
	var card = document.createElement('div');
	card.classList.add("whiteCard");
	var textNode = document.createTextNode(text);
	card.appendChild(textNode);

	card.onclick = function() {
		if (!hasSubmittedCard) {
			deselectCards();
			selectCard(this);
		}
	}

	handOfCardsContainer.appendChild(card);
}

window.onload = function() {
	handOfCardsContainer = document.getElementById("handOfCards");
	submitButton = document.getElementById("submitButton");

	addWhiteCard("White card from JS #1");
	addWhiteCard("White card from JS #2");
	addWhiteCard("White card from JS #3");
	addWhiteCard("White card from JS #4");
}

function getSelectedCard() {
	var whiteCards = document.getElementsByClassName("selectedCard");
	if (whiteCards.length > 0) {
		return whiteCards[0];
	} else {
		return null;
	}
}

function submitSelection() {
	if (hasSubmittedCard) {
		alert("You have already submitted your card!");
		return;
	}

	var selectedCard = getSelectedCard();
	if(selectedCard === null) {
		alert("No card is selected! please select one by clicking on it!");
	} else {
		selectedCard.classList.add("submittedCard");
		hasSubmittedCard = true;
		submitButton.disabled = true;

		var payload = JSON.stringify( { card_id: 123321 } );
		console.log("Sending over: " + payload);

		$.ajax({
		    url: 'submitCard',
		    // dataType: 'json', //Omitted because jquery trips over itself it I put it in
		    type: 'post',
		    contentType: 'application/json',
		    data: payload,
		    processData: false,
		})
		.done(function( data, textStatus, jQxhr ) {
			alert( "Submitted card response: " + jQxhr.status + " Data Loaded: '" + data + "'" );
		})
		.fail(function(request, status, error) {
			alert("ERROR Submitting card. Some info: " + request.responseText + " + " + error + " + " + status);
		})
		.always(function() {
			alert( "finished" );
		});


		// $.post( "submitCard", { cardId: 123321 })
		// .done(function( data, textStatus, xhr ) {
		// 	alert( "Submitted card response: " + xhr.status + " Data Loaded: '" + data + "'" );
		// })
		// .fail(function() {
		// 	alert( "error" );
		// })
		// .always(function() {
		// 	alert( "finished" );
		// });

	}
}

function newRoundStarts() {
	hasSubmittedCard = false;
	submitButton.disabled = false;
}